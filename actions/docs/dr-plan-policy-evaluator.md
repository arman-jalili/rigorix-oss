# Disaster Recovery Plan: policy-evaluator

**Module:** `actions/src/policy_evaluator/`
**Epic:** policy-evaluator
**Last Updated:** 2026-06-20

## Overview

The policy-evaluator module is stateless and has no persistent storage,
databases, or external caches. DR is primarily about ensuring continuity
of service when dependencies (GitHub API, diff-analyzer) fail.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | < 5 minutes | Stateless — restart is instant |
| RPO (Recovery Point Objective) | N/A | No persistent state to lose |

## Failure Scenarios

### Scenario 1: GitHub API Down

**Impact:** Cannot load policy file from base branch. Policy evaluation blocked.

**Symptoms:**
- `PolicyError::GitHubApi` or `PolicyError::FileNotFound`
- All policy-dependent operations fail

**Detection:**
- Error logged at ERROR level with repo and ref details
- Retry count exceeded (3 attempts with exponential backoff)

**Recovery Steps:**
1. Wait for GitHub to recover (check status.github.com)
2. For urgent evaluations, inject policy content directly via `parse_content()`
3. If outage exceeds 30 min, activate manual review process (bypass policy)

**Prevention:**
- Policy is loaded once per action run (not per file)
- Retry with exponential backoff built in
- Fail-open: default empty policy if GitHub is unreachable

### Scenario 2: Invalid/Corrupted Policy File

**Impact:** Policy parsing fails. No policy enforcement possible.

**Symptoms:**
- `PolicyError::InvalidSyntax` or `PolicyError::UnsupportedVersion`
- Policy loading fails before evaluation begins

**Detection:**
- Error logged at ERROR level with details
- Policy validation errors visible in action logs

**Recovery Steps:**
1. Revert the policy file to last known good version in git history
2. Re-run the action with the reverted policy
3. If policy file is corrupted on base branch, restore from `.git/` or backup

**Prevention:**
- Policy is version-controlled alongside code
- Each policy change goes through PR review
- Policy syntax is validated at load time

### Scenario 3: Diff Analyzer Failure

**Impact:** No PR diff to evaluate. Policy evaluation cannot proceed.

**Symptoms:**
- Upstream diff-analyzer module returns errors
- Empty or partial `PrDiff` passed to evaluator

**Detection:**
- Error propagated from diff-analyzer as `DiffAnalyzerError`
- Fallback to empty diff if diff-analyzer fails

**Recovery Steps:**
1. Restart the action — diffs are re-fetched each run
2. If diff-analyzer is down, the policy evaluator receives an empty diff
   and will report no violations (fail-open behavior)
3. Alert the team if diff-analyzer failures persist

**Prevention:**
- diff-analyzer has its own DR plan and retry logic
- Policy evaluator handles empty diffs gracefully

### Scenario 4: Configuration Error

**Impact:** Action misconfigured, wrong policy path, wrong merge strategy.

**Symptoms:**
- `PolicyError::InvalidLimits` or incorrect evaluation results
- Policy violations not matching expected rules

**Detection:**
- Action logs show policy loading from unexpected path
- Review count mismatch (org vs repo rules)

**Recovery Steps:**
1. Verify `policy_file` input in workflow YAML
2. Check `.rigorix/policy.toml` exists on correct branch
3. Verify org policy source URL/path
4. Re-run with corrected configuration

**Prevention:**
- All configuration is validated at load time
- Policy path defaults to `.rigorix/policy.toml`
- Merge strategy defaults to `restrictive` (safest option)

### Scenario 5: Performance Degradation

**Impact:** Policy evaluation takes too long (very large rulesets, complex patterns).

**Symptoms:**
- `evaluation_time_ms` exceeds expected threshold
- Action hits GitHub Actions 6-hour timeout

**Detection:**
- Metrics show high `policy_eval_time_ms`
- Action timeout approaching

**Recovery Steps:**
1. Reduce number of rules in policy file
2. Simplify glob patterns (avoid overly broad patterns like `**`)
3. Increase `max_files` limit if legitimate large diffs
4. Consider splitting policy into multiple files

**Prevention:**
- Policy evaluation is O(n*m) where n = files, m = rules
- Typical PR with 100 files and 20 rules evaluates in < 100ms
- Glob compilation is cached in `CompiledRules`

## Backup and Restore

### What to Back Up

| Asset | Backup Method | Frequency |
|-------|---------------|-----------|
| Policy files (`.rigorix/policy.toml`) | Git history | Every commit |
| Org policy files | Git history (org repo) | Every commit |

### Restore Procedure

1. Identify the last known good policy commit from git history:
   ```bash
   git log --oneline -- .rigorix/policy.toml
   ```
2. Restore the policy file:
   ```bash
   git checkout <last-good-commit> -- .rigorix/policy.toml
   ```
3. Commit and push the restored policy:
   ```bash
   git commit -m "revert: restore policy to last known good version"
   git push
   ```
4. Re-run the action to verify policy loads correctly

## Testing DR Procedures

| Test | Frequency | Procedure |
|-----|-----------|-----------|
| Policy load failure | Per release | Remove `.rigorix/policy.toml` and verify fail-open behavior |
| Invalid syntax recovery | Per release | Inject malformed TOML and verify error message |
| Orphan policy merge | Per release | Load policy without org source and verify warning |
| Empty diff handling | Per release | Evaluate empty `PrDiff` and verify zero violations result |
| Large ruleset test | Quarterly | Evaluate with 100+ rules and verify < 500ms processing time |
