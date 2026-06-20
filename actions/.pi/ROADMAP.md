# Actions Crate — Implementation Roadmap

<!--
Canonical Reference: .pi/ROADMAP.md
Blueprint Source: Rigorix design session (2026-06-20)
Total modules: 10 (9 action modules + 1 shared)
-->

## Overview

The `actions/` crate is a Rust binary that wraps `rigorix-engine` as a GitHub Action. It supports two modes:
- **Mode A**: Reactive governance — policy checks, diff analysis, audit posting
- **Mode B**: Active execution — code generation with validation loop

All business logic lives in `rigorix-engine`. The actions crate is a thin adapter with GitHub-specific I/O.

## Dependency Graph

```
                    ┌─────────────────┐
                    │  action-input   │  (standalone)
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
     ┌────────────┐  ┌────────────┐  ┌────────────┐
     │  security  │  │  diff-     │  │  action-   │
     │  -config   │  │  analyzer  │  │  output    │
     └─────┬──────┘  └─────┬──────┘  └─────┬──────┘
           │               │               │
           ▼               ▼               │
     ┌────────────┐  ┌────────────┐        │
     │  audit-    │  │  policy-   │        │
     │  posting   │  │  evaluator │        │
     └─────┬──────┘  └─────┬──────┘        │
           │               │               │
           └───────┬───────┘               │
                   │                       │
                   ▼                       ▼
           ┌────────────┐          ┌────────────┐
           │  ci-       │          │  action-   │
           │  integration│          │  entrypoint│ ◄── wires everything
           └────────────┘          └────────────┘
```

---

## Phase 1: Foundation (Week 1)

**Goal:** Scaffold the crate, build the shared infrastructure, parse GitHub inputs.

### 1.1 Crate Scaffolding

| Task | Deliverable | Depends On |
|------|-------------|------------|
| Create `actions/Cargo.toml` | Workspace member with `rigorix-engine` dependency | Nothing |
| Create `actions/src/lib.rs` | Module declarations | Cargo.toml |
| Create `action.yml` | GitHub Action definition with inputs/outputs | Nothing |
| Create `Dockerfile` | Container-based action | action.yml |

### 1.2 Shared: GitHubClient

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/src/shared/github_client.rs` | `GitHubClient` struct with token auth, rate limiting, Retry-After | Nothing |
| `actions/src/shared/mod.rs` | Module declaration | github_client.rs |
| Unit tests | Mock GitHub API responses | github_client.rs |

The `GitHubClient` is used by `security-config`, `diff-analyzer`, `ci-integration`, and `audit-posting`. Extract it as a shared module to avoid circular dependencies.

**Interface:**
```rust
pub struct GitHubClient {
    token: String,
    http: reqwest::Client,
}

impl GitHubClient {
    pub fn new(token: String) -> Self;
    pub async fn fetch_pr_diff(&self, repo: &str, pr: u64) -> Result<String>;
    pub async fn create_status(&self, repo: &str, sha: &str, status: GitHubStatus) -> Result<()>;
    pub async fn create_issue_comment(&self, repo: &str, issue: u64, body: &str) -> Result<Comment>;
    pub async fn update_issue_comment(&self, comment_id: u64, body: &str) -> Result<()>;
    pub async fn list_issue_comments(&self, repo: &str, issue: u64) -> Result<Vec<Comment>>;
    pub async fn add_labels(&self, repo: &str, issue: u64, labels: &[&str]) -> Result<()>;
    pub async fn remove_label(&self, repo: &str, issue: u64, label: &str) -> Result<()>;
    pub async fn read_file_from_ref(&self, repo: &str, path: &str, ref_name: &str) -> Result<String>;
}
```

### 1.3 action-input

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/src/action_input/parser.rs` | `InputParser` — reads `INPUT_*` env vars | Nothing |
| `actions/src/action_input/types.rs` | `ActionInputs`, `ActionConfig`, `GitHubEvent` | Nothing |
| `actions/src/action_input/event_parser.rs` | `EventPayloadParser` — parses `GITHUB_EVENT_PATH` | Nothing |
| `actions/src/action_input/comment_parser.rs` | `CommentParser` — `/rigorix` slash commands | Nothing |
| `actions/src/action_input/ci_detector.rs` | `CiDetector` — detects CI environment | Nothing |
| `actions/src/action_input/config_loader.rs` | `ConfigLoader` — merges action.yml defaults + env | Nothing |
| `actions/src/action_input/mod.rs` | Module root + re-exports | All submodules |
| Unit tests | 15+ tests covering all parsers | All submodules |

---

## Phase 2: Security (Week 1-2)

**Goal:** Phase 0 security validation — runs before any operation.

### 2.1 security-config

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/src/security_config/context.rs` | `SecurityContext`, `SecurityLevel` | Nothing |
| `actions/src/security_config/validator.rs` | `SecurityValidator::validate()` | All submodules |
| `actions/src/security_config/fork_detector.rs` | `ForkDetector` | Nothing |
| `actions/src/security_config/secret_masker.rs` | `SecretMasker` | Nothing |
| `actions/src/security_config/token_validator.rs` | `TokenValidator` | GitHubClient |
| `actions/src/security_config/url_allowlist.rs` | `UrlAllowlist` | Nothing |
| `actions/src/security_config/hmac_signer.rs` | `HmacSigner` — HMAC-SHA256 signing | Nothing (uses engine's hmac+sha2) |
| `actions/src/security_config/org_policy.rs` | `OrgPolicyLoader` | GitHubClient |
| `actions/src/security_config/error.rs` | `SecurityError` enum | Nothing |
| `actions/src/security_config/mod.rs` | Module root + re-exports | All submodules |
| Unit tests | 20+ tests (fork detection, URL validation, HMAC) | All submodules |

---

## Phase 3: Mode A — Reactive Governance (Week 2-3)

**Goal:** PR diff analysis + policy evaluation + governance reporting.

### 3.1 diff-analyzer

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/src/diff_analyzer/diff.rs` | `PrDiff` struct | Nothing |
| `actions/src/diff_analyzer/file.rs` | `ChangedFile`, `FileStatus`, `DiffHunk` | Nothing |
| `actions/src/diff_analyzer/parser.rs` | `DiffParser` — parses git diff output | Nothing |
| `actions/src/diff_analyzer/path_validator.rs` | `PathValidator` — traversal, symlink, binary | Nothing |
| `actions/src/diff_analyzer/limits.rs` | `LimitEnforcer` — max size/files/lines | Nothing |
| `actions/src/diff_analyzer/ai_signals.rs` | `AiSignalDetector` — heuristic AI detection | Nothing |
| `actions/src/diff_analyzer/risk.rs` | `RiskClassifier` — path-based risk levels | Nothing |
| `actions/src/diff_analyzer/error.rs` | `DiffAnalyzerError` enum | Nothing |
| `actions/src/diff_analyzer/mod.rs` | Module root + re-exports | All submodules |
| Unit tests | 25+ tests (path parsing, limit enforcement, risk classification) | All submodules |

### 3.2 policy-evaluator

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/src/policy_evaluator/policy.rs` | `PolicyDocument`, `PolicyRules`, `PolicyLimits` | Nothing |
| `actions/src/policy_evaluator/rule.rs` | `DenyRule`, `ReviewRule`, `FlagRule`, `Severity` | Nothing |
| `actions/src/policy_evaluator/loader.rs` | `PolicyLoader` — loads from base branch, detects tampering | GitHubClient |
| `actions/src/policy_evaluator/evaluator.rs` | `PolicyEvaluator` — matches files against rules | diff-analyzer |
| `actions/src/policy_evaluator/org_merger.rs` | `OrgPolicyMerger` — union rules, min limits | security-config |
| `actions/src/policy_evaluator/violation.rs` | `PolicyViolation` enum | Nothing |
| `actions/src/policy_evaluator/result.rs` | `PolicyResult` — has_blocking, has_warnings, counts | Nothing |
| `actions/src/policy_evaluator/error.rs` | `PolicyError` enum | Nothing |
| `actions/src/policy_evaluator/mod.rs` | Module root + re-exports | All submodules |
| Unit tests | 30+ tests (rule matching, policy loading, org merging) | All submodules |

### 3.3 action-output (Mode A: governance outputs)

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/src/action_output/formatter.rs` | `OutputFormatter` — orchestrates all output channels | Nothing |
| `actions/src/action_output/annotations.rs` | `AnnotationWriter` — `::error`/`::warning` workflow commands | policy-evaluator |
| `actions/src/action_output/summary.rs` | `StepSummaryWriter` — markdown summaries | Nothing |
| `actions/src/action_output/variables.rs` | `OutputVariableWriter` — `$GITHUB_OUTPUT` | Nothing |
| `actions/src/action_output/types.rs` | `ActionOutput` container | policy-evaluator, diff-analyzer |
| Unit tests | 15+ tests (annotation formatting, summary rendering) | All submodules |

---

## Phase 4: Mode B — Active Execution (Week 3-4)

**Goal:** Code generation with validation loop, CI integration, audit posting.

### 4.1 action-output (Mode B: execution outputs)

| Task | Deliverable | Depends On |
|------|-------------|------------|
| Extend `OutputFormatter` | Execution summary formatting (plan, validation, quality) | Phase 3.3 |
| Add validation report rendering | Collapsible `<details>` sections for iteration history | phase 3.3 |

### 4.2 ci-integration

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/src/ci_integration/status_check.rs` | `StatusCheckManager` — commit statuses | GitHubClient |
| `actions/src/ci_integration/pr_comment.rs` | `PrCommentManager` — sticky PR comments | GitHubClient, action-output |
| `actions/src/ci_integration/issue_labels.rs` | `IssueLabelManager` — apply/remove labels | GitHubClient |
| `actions/src/ci_integration/workflow_trigger.rs` | `WorkflowTrigger` — follow-up workflows | GitHubClient |
| `actions/src/ci_integration/execution_tracker.rs` | `ExecutionTracker` — per-PR idempotency | Nothing |
| `actions/src/ci_integration/error.rs` | `CiIntegrationError` enum | Nothing |
| `actions/src/ci_integration/mod.rs` | Module root + re-exports | All submodules |
| Unit tests | 20+ tests (status transitions, label management) | All submodules |

### 4.3 audit-posting

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/src/audit_posting/backend.rs` | `AuditBackend` trait (open-core boundary) | Nothing |
| `actions/src/audit_posting/record.rs` | `SignedAuditRecord`, `AuditRecord` | Nothing |
| `actions/src/audit_posting/signer.rs` | `AuditSigner` — wraps `HmacSigner` for records | security-config |
| `actions/src/audit_posting/filesystem_backend.rs` | `FilesystemAuditBackend` — OSS default | Nothing |
| `actions/src/audit_posting/noop_backend.rs` | `NoopAuditBackend` — dry-run/testing | Nothing |
| `actions/src/audit_posting/retry.rs` | `AuditRetryConfig` — backoff + jitter | Nothing |
| `actions/src/audit_posting/queue.rs` | `AuditRecordQueue` — offline resilience | Nothing |
| `actions/src/audit_posting/poster.rs` | `AuditPoster` — orchestrates post + retry + queue | All submodules |
| `actions/src/audit_posting/error.rs` | `AuditError` enum | Nothing |
| `actions/src/audit_posting/mod.rs` | Module root + re-exports | All submodules |
| Unit tests | 20+ tests (signing, filesystem backend, retry backoff) | All submodules |

---

## Phase 5: Integration (Week 4)

**Goal:** Wire everything together, create entry point, write workflow examples.

### 5.1 action-entrypoint

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/src/action_entrypoint/context.rs` | `ActionContext` — assembled from all inputs | action-input, security-config |
| `actions/src/action_entrypoint/mode.rs` | `ActionMode` enum | Nothing |
| `actions/src/action_entrypoint/router.rs` | `ActionRouter` — event → engine call | All modules |
| `actions/src/action_entrypoint/error.rs` | `ActionError` — exit codes | Nothing |
| `actions/src/action_entrypoint/main.rs` | Binary entry point | All modules |
| `actions/src/action_entrypoint/mod.rs` | Module root + re-exports | All submodules |
| `actions/src/main.rs` | `fn main()` — tokio runtime, router dispatch | action-entrypoint |

### 5.2 Integration Tests

| Task | Deliverable | Depends On |
|------|-------------|------------|
| Mode A integration test | PR diff → policy check → annotations → audit | Phase 3 |
| Mode B integration test | Intent → validate → generate → commit | Phase 4 |
| Mock GitHub API | Test fixtures for API responses | GitHubClient |
| CI pipeline | `.github/workflows/rigorix.yml` for self-testing | action-entrypoint |

### 5.3 Documentation

| Task | Deliverable | Depends On |
|------|-------------|------------|
| `actions/README.md` | Usage guide, examples, configuration reference | All modules |
| `actions/USAGE.md` | Mode A and Mode B workflow examples | All modules |
| `actions/CONTRIBUTING.md` | Plugin development guide (AuditBackend) | audit-posting |

---

## Summary Timeline

```
Week 1:  ████████░░░░░░░░░░░░  Phase 1 (Foundation) + Phase 2 (Security)
Week 2:  ████████████████░░░░  Phase 3 (Mode A: diff-analyzer + policy-evaluator)
Week 3:  ████████████████████  Phase 3 (Mode A: output) + Phase 4 (Mode B: output + ci)
Week 4:  ████████████████████  Phase 4 (audit-posting) + Phase 5 (Integration)
```

### Module Count & Test Targets

| Phase | Modules | Source Files (est.) | Tests (est.) |
|-------|---------|--------------------|--------------|
| 1: Foundation | 3 (shared + input + scaffold) | 12 | 25 |
| 2: Security | 1 (security-config) | 9 | 20 |
| 3: Mode A | 3 (diff-analyzer + policy-evaluator + output) | 20 | 70 |
| 4: Mode B | 2 (ci-integration + audit-posting) | 15 | 40 |
| 5: Integration | 1 (action-entrypoint) | 5 | 15 |
| **Total** | **10** | **~61** | **~170** |

---

*Last updated: 2026-06-20*
*Blueprint: actions/.pi/ROADMAP.md*
