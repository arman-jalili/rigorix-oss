# Runbook: policy-evaluator

**Module:** `actions/src/policy_evaluator/`
**Epic:** policy-evaluator
**Last Updated:** 2026-06-20

## Overview

The policy-evaluator module implements Mode A reactive governance for Rigorix.
It checks Pull Request diffs against a configurable policy file (`.rigorix/policy.toml`)
and classifies violations into three categories: deny (blocks the PR), require_review
(flags for human review), and flag (warns without blocking). Policies are loaded from
the **base branch** (not the PR) to prevent tampering.

## Startup Sequence

### Dependencies

| Dependency | Type | Required | Description |
|-----------|------|----------|-------------|
| GitHub API | External | Yes | Reading policy file from base branch |
| rigorix-engine | Internal | Yes | Core engine types |
| diff-analyzer | Internal | Yes | `PrDiff` struct for changed file iteration |
| security-config | Internal | No | Organization policy path for merging |
| serde | Cargo | Yes | Serialization |
| async-trait | Cargo | Yes | Async trait support |
| globset | Cargo | Yes | Glob pattern matching |
| toml | Cargo | Yes | Policy file parsing |

### Startup Procedure

1. **Policy loading**: The `PolicyLoadingService` reads `.rigorix/policy.toml` from
   the base branch. If the file doesn't exist, a default policy with no rules is used.
2. **Service initialization**:
   ```rust
   use policy_evaluator::application::policy_evaluation_pipeline_impl::PolicyEvaluationPipelineServiceImpl;

   let pipeline = PolicyEvaluationPipelineServiceImpl;
   ```
3. **Initialization check**: Call `pipeline.run()` with an empty diff to verify
   all services initialize without processing real data.

## Graceful Shutdown

The policy-evaluator module is **stateless** — there are no in-memory caches,
open connections, or background tasks to clean up. Shutdown is immediate.

Policy loading uses a `read_policy_content()` call that should respect cancellation
tokens when available. The evaluation step is deterministic and completes within
milliseconds for typical PRs (< 500 files).

## Common Failure Modes

### 1. Policy File Not Found

**Symptoms:** `PolicyError::FileNotFound` returned.

**Causes:**
- `.rigorix/policy.toml` does not exist in the base branch
- Policy path is misconfigured in action inputs

**Recovery:**
- The action will fail-open (warn-only) by default
- Use compiled-in defaults or empty policy to proceed
- If `fail_on_action_error` is set, the action will fail

### 2. Invalid Policy Syntax

**Symptoms:** `PolicyError::InvalidSyntax` returned.

**Causes:**
- Malformed TOML in `.rigorix/policy.toml`
- Invalid field names or types
- Missing required fields

**Recovery:**
- Log the error with line number details
- Fail the action (invalid policy is a blocking error)
- The user must fix the policy file and re-run

### 3. Duplicate Rule Names

**Symptoms:** `PolicyError::DuplicateRuleName` returned.

**Causes:**
- Two rules with the same `name` field across deny/review/flag categories

**Recovery:**
- Fail the action — duplicate names cause ambiguous results
- The user must rename one of the rules

### 4. Invalid Glob Pattern

**Symptoms:** `PolicyError::InvalidGlobPattern` returned.

**Causes:**
- A rule's `pattern` field contains an invalid glob expression
- Common mistakes: unclosed brackets (`[invalid`), invalid escape sequences

**Recovery:**
- Fail the action with the specific rule name and pattern
- The user must fix the pattern and re-run

### 5. Organization Policy Not Found

**Symptoms:** `PolicyError::OrgPolicyLoadError` returned.

**Causes:**
- Organization policy source is unreachable (network error)
- Policy file doesn't exist at the configured org path

**Recovery:**
- If `require_org_policy` is false (default): warn and proceed with repo policy only
- If `require_org_policy` is true: fail the action

### 6. GitHub API Error

**Symptoms:** `PolicyError::GitHubApi` returned.

**Causes:**
- Network timeout fetching policy file
- Rate limiting (GitHub API)
- Repository not found or access denied

**Recovery:**
- Retry with exponential backoff (max 3 retries)
- Check rate limit headers
- If unrecoverable, fail-open with default policy

## Configuration Reference

### Policy Files

| Setting | Default | Description |
|---------|---------|-------------|
| policy file path | `.rigorix/policy.toml` | Path to TOML policy file |
| org policy path | `.github/rigorix/org-policy.toml` | Path to org-level policy |
| merge strategy | `restrictive` | How org+repo policies are merged |

### Policy Document Fields

| Field | Type | Description |
|-------|------|-------------|
| `version` | String | Policy schema version (semver) |
| `rules.deny` | Array | Deny rules (blocking) |
| `rules.require_review` | Array | Review rules (non-blocking) |
| `rules.flag` | Array | Flag rules (warnings) |
| `limits` | Table | Resource limits |
| `audit` | Table | Audit configuration |

### Action Configuration

| Input | Default | Description |
|-------|---------|-------------|
| `fail_on_violation` | `false` | Fail workflow on violations |
| `policy_file` | `.rigorix/policy.toml` | Custom policy path |

## Monitoring

### Key Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| policy_load_time_ms | `PolicyLoadingService.load()` | Time to load and parse policy |
| policy_eval_time_ms | `PolicyEvaluationService.evaluate()` | Time to evaluate diff |
| policy_merge_time_ms | `OrgPolicyMergingService.merge()` | Time to merge policies |
| total_files_evaluated | Pipeline | Files checked against rules |
| total_violations | Pipeline | Total violations found |
| total_blocking_violations | Pipeline | Deny violations found |
| total_tamper_detected | Counter | Policy tamper events |

### Log Levels

| Level | Usage |
|-------|-------|
| ERROR | Parse failures, invalid syntax, GitHub API errors |
| WARN | Tamper detected, org policy not found, limits exceeded |
| INFO | Policy loaded, evaluation start/complete, violation summary |
| DEBUG | Individual file matching, rule evaluation results |
| TRACE | Raw policy content, compiled glob patterns |
