# Policy Evaluator Architecture

<!--
Canonical Reference: .pi/architecture/modules/policy-evaluator.md
Blueprint Source: Ported from original Rigorix docs/ARCHITECTURE_GITHUB_ACTIONS.md §2.2 (2026-04-27)
Rationale: Mode A reactive governance — enforces deny/review/flag rules against PR diffs
-->

## Overview

Mode A of the Rigorix GitHub Action. The Policy Evaluator checks Pull Request diffs against a configurable policy file (`.rigorix/policy.toml`) and classifies violations into three categories: deny (blocks the PR), require_review (flags for human review), and flag (warns without blocking). Policies are loaded from the **base branch** (not the PR) to prevent tampering.

This is the governance layer — it checks code **after** it's written, complementing Mode B which **generates** code.

## Philosophy

The Policy Evaluator is deterministic, not LLM-driven. It uses glob pattern matching and rule-based classification. Policy files are TOML documents versioned alongside code. Organization-wide policies can be merged with repository-level policies — the most restrictive rule always wins.

## Responsibilities

- Load `.rigorix/policy.toml` from the repository (base branch, not PR)
- Detect if the PR itself modified the policy file (tamper detection)
- Merge organization-level policy with repository-level policy
- Parse PR diffs and match changed files against glob rules
- Classify matches into deny / require_review / flag with severity
- Support glob patterns: `migrations/**`, `src/auth/*.rs`, `*.sql`
- Emit GitHub workflow annotations for each violation
- Return structured results for the report generator and status check
- Fail-open by default: policy violations are warnings unless `fail_on_violation` is set

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| PolicyDocument | `actions/src/policy_evaluator/policy.rs` | Policy struct: version, rules, limits, audit config | #policy |
| PolicyRule | `actions/src/policy_evaluator/rule.rs` | DenyRule, ReviewRule, FlagRule with glob patterns | #rule |
| PolicyLoader | `actions/src/policy_evaluator/loader.rs` | Loads policy from base branch, detects tampering | #loader |
| PolicyEvaluator | `actions/src/policy_evaluator/evaluator.rs` | Matches changed files against rules, returns violations | #evaluator |
| OrgPolicyMerger | `actions/src/policy_evaluator/org_merger.rs` | Merges org-level policy with repo policy | #merger |
| PolicyViolation | `actions/src/policy_evaluator/violation.rs` | Structured violation with file, rule, severity, message | #violation |
| PolicyResult | `actions/src/policy_evaluator/result.rs` | Aggregate result: has_blocking, has_warnings, violations list | #result |
| PolicyError | `actions/src/policy_evaluator/error.rs` | Typed errors: FileNotFound, InvalidSyntax, Evaluation | #error |

---

## Component Details

### PolicyDocument

```rust
/// Complete policy document loaded from `.rigorix/policy.toml`.
///
/// Policies are versioned and include rules for deny, review, and flag
/// classifications, plus resource limits and audit configuration.
#[derive(Debug, Deserialize)]
pub struct PolicyDocument {
    /// Policy schema version (semver).
    pub version: String,

    /// Classification rules.
    #[serde(default)]
    pub rules: PolicyRules,

    /// Resource limits (max diff size, max files).
    #[serde(default)]
    pub limits: PolicyLimits,

    /// Audit configuration (HMAC key path, backend URL).
    #[serde(default)]
    pub audit: AuditConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct PolicyRules {
    /// Rules that block the PR (commit status: failure).
    #[serde(rename = "deny", default)]
    pub deny_rules: Vec<DenyRule>,

    /// Rules that require human review (commit status: neutral).
    #[serde(rename = "require_review", default)]
    pub require_review_rules: Vec<ReviewRule>,

    /// Rules that warn without blocking (commit status: success with annotations).
    #[serde(rename = "flag", default)]
    pub flag_rules: Vec<FlagRule>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PolicyLimits {
    /// Maximum PR diff size in bytes (default: 1MB).
    pub max_diff_size: Option<u64>,

    /// Maximum number of changed files (default: 500).
    pub max_files: Option<usize>,

    /// Maximum changed lines per file (default: 2000).
    pub max_lines_per_file: Option<usize>,
}
```

### PolicyRule Types

```rust
/// A deny rule: if a changed file matches this pattern, the PR is blocked.
#[derive(Debug, Deserialize)]
pub struct DenyRule {
    pub name: String,
    pub description: String,
    /// Glob pattern matching file paths (e.g., "migrations/**", "*.sql").
    pub pattern: String,
    /// Severity: critical (always block), high (block unless overridden).
    pub severity: Severity,
    /// Optional: only apply when the committing user is NOT in the exclude list.
    #[serde(default)]
    pub exclude_users: Vec<String>,
}

/// A review rule: matching files require manual review before merge.
#[derive(Debug, Deserialize)]
pub struct ReviewRule {
    pub name: String,
    pub description: String,
    pub pattern: String,
    /// Number of required reviewers for matching files.
    #[serde(default = "default_reviewers")]
    pub required_reviewers: u8,
}

/// A flag rule: matching files generate a warning annotation but don't block.
#[derive(Debug, Deserialize)]
pub struct FlagRule {
    pub name: String,
    pub description: String,
    pub pattern: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

fn default_reviewers() -> u8 { 1 }
```

### PolicyLoader

```rust
/// Loads policy from the repository's base branch.
///
/// Security: Policy MUST be loaded from the BASE BRANCH (e.g., `origin/main`),
/// not from the PR branch. This prevents attackers from modifying the policy
/// to bypass governance checks.
pub struct PolicyLoader;

impl PolicyLoader {
    /// Load policy from the base branch.
    ///
    /// 1. Checkout the base branch policy file
    /// 2. Validate TOML syntax
    /// 3. Compile glob patterns
    /// 4. Validate rule consistency
    pub async fn load(
        policy_path: &str,
        base_ref: &str,
    ) -> Result<PolicyDocument, PolicyError> {
        // Read policy from base branch
        let content = Self::read_from_base(policy_path, base_ref).await?;

        // Parse
        let policy: PolicyDocument = toml::from_str(&content)
            .map_err(|e| PolicyError::InvalidSyntax(e.to_string()))?;

        // Validate version
        Self::validate_version(&policy.version)?;

        // Validate glob patterns compile
        Self::validate_patterns(&policy.rules)?;

        Ok(policy)
    }

    /// Detect if the PR modifies the policy file itself.
    pub fn detect_policy_tamper(
        pr_diff: &PrDiff,
        policy_path: &str,
    ) -> bool {
        pr_diff.changed_files().any(|f| f.path == policy_path)
    }
}
```

### PolicyEvaluator

```rust
/// Evaluates PR diff against policy rules.
pub struct PolicyEvaluator {
    policy: PolicyDocument,
    compiled_rules: CompiledRules,
}

impl PolicyEvaluator {
    /// Evaluate a PR diff against all policy rules.
    pub fn evaluate(&self, diff: &PrDiff) -> PolicyResult {
        let mut violations = Vec::new();

        for file in diff.changed_files() {
            // Check deny rules
            for rule in &self.compiled_rules.deny {
                if rule.matches(&file.path) {
                    violations.push(PolicyViolation::Deny {
                        rule: rule.name.clone(),
                        description: rule.description.clone(),
                        severity: rule.severity.clone(),
                        file: file.path.clone(),
                        message: format!(
                            "File '{}' matches deny rule '{}': {}",
                            file.path, rule.name, rule.description
                        ),
                    });
                }
            }

            // Check review rules
            for rule in &self.compiled_rules.review {
                if rule.matches(&file.path) {
                    violations.push(PolicyViolation::RequireReview {
                        rule: rule.name.clone(),
                        description: rule.description.clone(),
                        file: file.path.clone(),
                        required_reviewers: rule.required_reviewers,
                    });
                }
            }

            // Check flag rules
            for rule in &self.compiled_rules.flag {
                if rule.matches(&file.path) {
                    violations.push(PolicyViolation::Flag {
                        rule: rule.name.clone(),
                        description: rule.description.clone(),
                        file: file.path.clone(),
                        message: rule.message.clone().unwrap_or_else(|| {
                            format!("File '{}' flagged by rule '{}'", file.path, rule.name)
                        }),
                    });
                }
            }
        }

        PolicyResult::new(violations)
    }
}

#[derive(Debug)]
pub struct PolicyResult {
    pub violations: Vec<PolicyViolation>,
}

impl PolicyResult {
    /// Returns true if any deny violations would block the PR.
    pub fn has_blocking_violations(&self) -> bool {
        self.violations.iter().any(|v| matches!(v, PolicyViolation::Deny { .. }))
    }

    /// Returns true if there are review-required or flag violations.
    pub fn has_warnings(&self) -> bool {
        self.violations.iter().any(|v| {
            matches!(v, PolicyViolation::RequireReview { .. } | PolicyViolation::Flag { .. })
        })
    }

    /// Count violations by type.
    pub fn count_by_type(&self) -> ViolationCounts {
        ViolationCounts {
            deny: self.violations.iter().filter(|v| matches!(v, PolicyViolation::Deny { .. })).count(),
            review: self.violations.iter().filter(|v| matches!(v, PolicyViolation::RequireReview { .. })).count(),
            flag: self.violations.iter().filter(|v| matches!(v, PolicyViolation::Flag { .. })).count(),
        }
    }
}
```

### OrgPolicyMerger

```rust
/// Merges organization-level policy with repository policy.
///
/// Merge rule: the most restrictive rule wins.
/// - Org deny rules are ADDED to repo deny rules
/// - Org review minimums are MAX of org and repo
/// - Org limits are MIN of org and repo (tighter bounds win)
pub struct OrgPolicyMerger;

impl OrgPolicyMerger {
    pub fn merge(
        repo_policy: &PolicyDocument,
        org_policy: &PolicyDocument,
    ) -> Result<PolicyDocument, PolicyError> {
        let mut merged = repo_policy.clone();

        // Deny rules: union (org rules always apply)
        merged.rules.deny_rules.extend(org_policy.rules.deny_rules.clone());

        // Review rules: union
        merged.rules.require_review_rules.extend(org_policy.rules.require_review_rules.clone());

        // Flag rules: union
        merged.rules.flag_rules.extend(org_policy.rules.flag_rules.clone());

        // Limits: tighter of org and repo
        if let (Some(repo_lim), Some(org_lim)) = (&repo_policy.limits.max_diff_size, &org_policy.limits.max_diff_size) {
            merged.limits.max_diff_size = Some((*repo_lim).min(*org_lim));
        }
        if let (Some(repo_lim), Some(org_lim)) = (&repo_policy.limits.max_files, &org_policy.limits.max_files) {
            merged.limits.max_files = Some((*repo_lim).min(*org_lim));
        }

        Ok(merged)
    }
}
```

---

## Data Flow

```
PR opened or synchronized
        │
        ▼
PolicyLoader::load(".rigorix/policy.toml", "origin/main")
  - Reads policy from BASE BRANCH (not PR)
  - Compiles glob patterns
  - Validates TOML syntax
        │
        ▼
PolicyLoader::detect_policy_tamper(pr_diff, ".rigorix/policy.toml")
  - If true → warn "Policy file modified — requires admin review"
        │
        ▼
OrgPolicyMerger::merge(repo_policy, org_policy)
  - Unions deny/review/flag rules
  - Mins resource limits (tighter wins)
        │
        ▼
PolicyEvaluator::evaluate(pr_diff)
  - For each changed file:
    - Match against deny rules → Deny violations
    - Match against review rules → RequireReview violations
    - Match against flag rules → Flag violations
        │
        ▼
PolicyResult { violations, has_blocking, has_warnings }
        │
        ├─→ Commit status: failure/neutral/success
        ├─→ Workflow annotations: ::error/::warning per violation
        └─→ PR comment: governance report
```

---

## Policy File Example

```toml
# .rigorix/policy.toml
version = "1.0.0"

[rules.deny]
[[rules.deny]]
name = "no-raw-sql-in-migrations"
description = "Raw SQL in migrations requires DBA review"
pattern = "migrations/**/*.sql"
severity = "critical"

[[rules.deny]]
name = "no-secrets-in-config"
description = "Secrets must use environment variables, never hardcoded"
pattern = "**/*.env"
severity = "critical"

[rules.require_review]
[[rules.require_review]]
name = "auth-changes-need-review"
description = "Authentication changes require security review"
pattern = "src/auth/**"
required_reviewers = 2

[rules.flag]
[[rules.flag]]
name = "large-migration-flag"
description = "Large migration files should be reviewed for performance impact"
pattern = "migrations/**/*.sql"
message = "Migration file exceeds 100 lines — verify performance impact"

[limits]
max_diff_size = 1_048_576
max_files = 500
max_lines_per_file = 2000
```

---

## Dependencies

### Depends On
- **diff-analyzer**: `PrDiff` struct for changed file iteration
- **security-config**: Organization policy path for merging
- **GitHub API**: Reading base branch content (via `GitHubClient`)

### Used By
- **action-entrypoint**: Mode A dispatch calls `PolicyEvaluator`
- **audit-posting**: Policy violations included in audit records
- **ci-integration**: Violations feed into annotations and status checks

---

## Related ADRs

- **Engine ADR-001** (`engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md`): Clean Architecture
- **Actions ADR-101** (`actions/.pi/architecture/decisions/ADR-101-actions-as-thin-adapter.md`): Policy evaluator is external to engine
- **Actions ADR-102** (`actions/.pi/architecture/decisions/ADR-102-github-event-routing.md`): PR opened → Mode A governance dispatch

---

*Last updated: 2026-06-20*
*Module version: 1.0.0 (Implemented)*
*Ported from: original Rigorix docs/ARCHITECTURE_GITHUB_ACTIONS.md §2.2*

---

**Status:** Implemented
**Engine modules reused:** configuration (policy.toml format), enforcement (rule patterns)

## Verification

| Check | Status |
|-------|--------|
| Build (`cargo build`) | ✅ |
| Test suite (41 tests) | ✅ All pass |
| Contract validation | ✅ 14/14 checks pass |
| Clippy | ✅ Clean |
| Format | ✅ Clean |

## Related Documents

- **Runbook:** `actions/docs/runbook-policy-evaluator.md`
- **DR Plan:** `actions/docs/dr-plan-policy-evaluator.md`
- **Contract Check:** `actions/.pi/scripts/ci/check_policy-evaluator_contracts.sh`
- **Coverage Check:** `actions/.pi/scripts/ci/check_policy-evaluator_coverage.sh`
- **Proofing Stage:** `actions/.pi/scripts/ci/stage_policy-evaluator_proofing.sh`
