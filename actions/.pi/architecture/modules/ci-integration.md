# CI Integration Architecture

<!--
Canonical Reference: .pi/architecture/modules/ci-integration.md
Blueprint Source: Rigorix design session (2026-06-20)
Rationale: CI-specific GitHub integration — status checks, PR comments, issue labeling, workflow orchestration
-->

## Overview

The CI Integration module provides GitHub-specific features that bridge engine execution with GitHub's CI/CD primitives. It handles PR status checks, automated PR comments with execution summaries, issue labeling based on outcomes, and multi-workflow orchestration.

## Philosophy

The engine doesn't know about GitHub. This module is the adapter that maps GitHub concepts (status checks, PR reviews, issue labels) to engine execution outcomes. When the engine succeeds, a green status check appears. When validation fails, annotations appear on the PR diff with precise line locations.

## Responsibilities

- Create and update GitHub commit status checks
- Post structured PR review comments with execution summaries
- Add/remove issue labels based on execution outcomes
- Handle `/rigorix` command responses in issue comments
- Trigger follow-up workflows based on engine results
- Manage GitHub API rate limits and retries
- Track execution history per PR for idempotency

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| StatusCheckManager | `actions/src/ci_integration/status_check.rs` | Creates/updates GitHub commit statuses | #status-check |
| PrCommentManager | `actions/src/ci_integration/pr_comment.rs` | Posts structured PR review comments | #pr-comment |
| IssueLabelManager | `actions/src/ci_integration/issue_labels.rs` | Manages issue labels based on outcomes | #labels |
| WorkflowTrigger | `actions/src/ci_integration/workflow_trigger.rs` | Triggers follow-up workflows | #workflow-trigger |
| GitHubClient | `actions/src/ci_integration/github_client.rs` | GitHub REST API client wrapper | #client |
| ExecutionTracker | `actions/src/ci_integration/execution_tracker.rs` | Tracks execution history per PR for idempotency | #tracker |

---

## Component Details

### StatusCheckManager

**Purpose:** Creates and updates GitHub commit status checks

```rust
/// Manages GitHub commit status checks for engine executions.
///
/// Maps engine execution states to GitHub status check states:
/// - Pending/Running → "pending"
/// - Completed/Validated → "success"
/// - Failed/Exhausted → "failure"
/// - PartialFailure → "error" (with annotations)
pub struct StatusCheckManager {
    client: Arc<GitHubClient>,
    context_prefix: String,
}

impl StatusCheckManager {
    /// Create a pending status check when execution starts.
    pub async fn create_pending(
        &self,
        commit_sha: &str,
        execution_id: Uuid,
        description: &str,
    ) -> Result<(), CiIntegrationError> {
        self.client.create_status(
            commit_sha,
            GitHubStatus {
                state: "pending",
                target_url: Some(self.execution_url(execution_id)),
                description: description.to_string(),
                context: format!("{}/execution", self.context_prefix),
            },
        ).await
    }

    /// Update status check on completion.
    pub async fn update_status(
        &self,
        commit_sha: &str,
        execution_id: Uuid,
        outcome: &ValidationOutcome,
    ) -> Result<(), CiIntegrationError> {
        let (state, description) = match outcome {
            ValidationOutcome::Validated { .. } => ("success", "All validations passed"),
            ValidationOutcome::Failed { report, .. } => (
                "failure",
                &format!("Validation failed after {} iterations", report.iterations),
            ),
            ValidationOutcome::PartialRecovery { .. } => (
                "error",
                "Partial recovery — some nodes recovered, others failed",
            ),
        };

        self.client.create_status(
            commit_sha,
            GitHubStatus {
                state,
                target_url: Some(self.execution_url(execution_id)),
                description: description.to_string(),
                context: format!("{}/execution", self.context_prefix),
            },
        ).await
    }
}

struct GitHubStatus {
    state: &'static str,   // "pending", "success", "failure", "error"
    target_url: Option<String>,
    description: String,
    context: String,
}
```

### PrCommentManager

**Purpose:** Posts structured PR review comments

```rust
/// Posts and updates PR comments with execution summaries.
///
/// Uses a "sticky comment" pattern: identifies the existing rigorix
/// comment and updates it in-place rather than posting multiple comments.
pub struct PrCommentManager {
    client: Arc<GitHubClient>,
    bot_identifier: String,  // "[rigorix-bot]" marker in comment body
}

impl PrCommentManager {
    /// Post or update the execution summary comment on a PR.
    pub async fn upsert_execution_comment(
        &self,
        pr_number: u64,
        summary: &str,
    ) -> Result<(), CiIntegrationError> {
        // Find existing rigorix comment
        let existing = self.find_bot_comment(pr_number).await?;

        if let Some(comment_id) = existing {
            self.client.update_issue_comment(comment_id, summary).await?;
        } else {
            self.client.create_issue_comment(pr_number, summary).await?;
        }

        Ok(())
    }

    /// Find the existing rigorix bot comment on a PR/issue.
    async fn find_bot_comment(&self, pr_number: u64) -> Option<u64> {
        let comments = self.client.list_issue_comments(pr_number).await.ok()?;
        comments.iter()
            .find(|c| c.body.contains(&self.bot_identifier))
            .map(|c| c.id)
    }
}
```

**PR comment format:**

```markdown
<!-- rigorix-bot -->
## 🤖 Rigorix Execution Summary

**Execution:** `e1852176` | **Status:** ✅ Passed | **Quality:** workspace

### Plan
| Step | Status | Duration |
|------|--------|----------|
| read-task-file | ✅ | 0.3s |
| add-get-active-tasks-method | ✅ | 1.2s |
| generate-test | ✅ | 3.8s |
| compile-check | ✅ | 4.5s |
| run-tests | ✅ | 2.6s |

**Validation:** 1 iteration | **Tokens:** 3,240 | **Template:** `add-get-active-tasks`

> Reply `/rigorix retry e1852176` to re-run this execution.
```

### IssueLabelManager

**Purpose:** Manages issue labels based on execution outcomes

```rust
/// Adds and removes issue labels based on execution outcomes.
///
/// Label mapping:
/// - Validation passed → `rigorix:verified`
/// - Validation failed → `rigorix:needs-fix`
/// - Compile error → `rigorix:compile-error`
/// - Test failure → `rigorix:test-failure`
/// - Template needs review → `rigorix:review-template`
pub struct IssueLabelManager {
    client: Arc<GitHubClient>,
}

impl IssueLabelManager {
    /// Apply labels based on validation outcome.
    pub async fn apply_outcome_labels(
        &self,
        issue_number: u64,
        outcome: &ValidationOutcome,
        failures: &[TemplateFailure],
    ) -> Result<(), CiIntegrationError> {
        // Remove previous rigorix labels
        self.remove_rigorix_labels(issue_number).await?;

        match outcome {
            ValidationOutcome::Validated { .. } => {
                self.add_label(issue_number, "rigorix:verified").await?;
            }
            ValidationOutcome::Failed { .. } => {
                self.add_label(issue_number, "rigorix:needs-fix").await?;
            }
            _ => {}
        }

        // Add specific failure labels
        for failure in failures {
            match failure {
                TemplateFailure::CompileError { .. } => {
                    self.add_label(issue_number, "rigorix:compile-error").await?;
                }
                TemplateFailure::TestFailure { .. }
                | TemplateFailure::AssertionFailure { .. } => {
                    self.add_label(issue_number, "rigorix:test-failure").await?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
```

### GitHubClient

**Purpose:** Lightweight GitHub REST API client wrapper

```rust
/// Minimal GitHub REST API client.
///
/// Uses `reqwest` with the GitHub token for authentication.
/// Handles rate limit headers and retries with exponential backoff.
pub struct GitHubClient {
    token: String,
    base_url: String,  // "https://api.github.com"
    http: reqwest::Client,
}

impl GitHubClient {
    pub fn new(token: String) -> Self { ... }

    // Status checks
    pub async fn create_status(&self, sha: &str, status: GitHubStatus) -> Result<(), CiIntegrationError> { ... }

    // Issue comments
    pub async fn create_issue_comment(&self, issue: u64, body: &str) -> Result<Comment, CiIntegrationError> { ... }
    pub async fn update_issue_comment(&self, comment_id: u64, body: &str) -> Result<(), CiIntegrationError> { ... }
    pub async fn list_issue_comments(&self, issue: u64) -> Result<Vec<Comment>, CiIntegrationError> { ... }

    // Labels
    pub async fn add_labels(&self, issue: u64, labels: &[&str]) -> Result<(), CiIntegrationError> { ... }
    pub async fn remove_label(&self, issue: u64, label: &str) -> Result<(), CiIntegrationError> { ... }
    pub async fn list_labels(&self, issue: u64) -> Result<Vec<Label>, CiIntegrationError> { ... }
}
```

---

## Workflow Example

```yaml
# .github/workflows/rigorix.yml
name: Rigorix

on:
  workflow_dispatch:
    inputs:
      intent:
        description: 'What should Rigorix do?'
        required: true
      mode:
        description: 'Execution mode'
        required: false
        default: 'validate'
  issue_comment:
    types: [created]
  pull_request:
    types: [opened, synchronize]

jobs:
  rigorix:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pull-requests: write
      issues: write
      statuses: write

    steps:
      - uses: actions/checkout@v4

      - name: Rigorix
        uses: rigorix/action@v1
        with:
          intent: ${{ github.event.inputs.intent }}
          mode: ${{ github.event.inputs.mode || 'validate' }}
          post-pr-comment: 'true'
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Follow-up (on failure)
        if: failure()
        uses: rigorix/action@v1
        with:
          mode: 'status'
```

---

## Dependencies

### Depends On
- **rigorix-engine::plan_validation**: `ValidationOutcome`, `ValidationReport` for statuses and labels
- **rigorix-engine::failure_parser**: `TemplateFailure` for failure-specific labeling
- **action-output**: PR comment formatting
- **GitHub REST API**: Commit statuses, issue comments, labels

### Used By
- **action-entrypoint**: Calls CI integration managers after engine dispatch
- **GitHub Actions workflow**: Receives status checks and PR comments

---

## Security Considerations

| Concern | Mitigation |
|---------|------------|
| GitHub token scope | Requires `contents: write`, `pull-requests: write`, `issues: write`, `statuses: write` — minimal scope |
| Token in logs | Token read from `secrets.GITHUB_TOKEN`, never interpolated into log messages |
| API rate limiting | Exponential backoff with jitter; respects `X-RateLimit-Remaining` headers |
| PR comment spam | "Sticky comment" pattern — single comment updated, not new comments |

---

## Implementation Notes

- File paths (`actions/src/ci_integration/`) are provisional until the `actions/` crate is scaffolded with `Cargo.toml` and `src/lib.rs`.
- The `GitHubClient` wraps `reqwest` (already a dependency of `rigorix-engine`). No new HTTP client is needed.
- Status check contexts use the prefix `rigorix/` (e.g., `rigorix/execution`, `rigorix/validation`).

---

## Related ADRs

- **Engine ADR-001** (`engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md`): CI integration is an adapter over engine services
- **Engine ADR-004** (`engine/.pi/architecture/decisions/ADR-004-autonomy-presets.md`): Autonomous CI feedback without human interaction
- **Actions ADR-103** (`actions/.pi/architecture/decisions/ADR-103-ci-permission-mode.md`): CI permission mode defaults

---

*Last updated: 2026-06-20*
*Module version: 1.0.0 (Planned)*

---

**Status:** Planned
**Engine modules reused:** plan_validation, failure_parser, action-output
