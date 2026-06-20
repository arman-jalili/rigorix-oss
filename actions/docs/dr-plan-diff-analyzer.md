# Disaster Recovery Plan: diff-analyzer

**Module:** `actions/src/diff_analyzer/`
**Epic:** diff-analyzer
**Last Updated:** 2026-06-20

## Overview

The diff-analyzer module is stateless and has no persistent storage, databases,
or external caches. DR is primarily about ensuring continuity of service when
dependencies (GitHub API, rigorix-engine) fail.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | < 5 minutes | Stateless — restart is instant |
| RPO (Recovery Point Objective) | N/A | No persistent state to lose |

## Failure Scenarios

### Scenario 1: GitHub API Down

**Impact:** Cannot fetch PR diffs. Diff analysis is blocked.

**Symptoms:**
- `DiffAnalyzerError::DiffFetchError` or `DiffAnalyzerError::GitHubApi`
- HTTP 500/503 from GitHub

**Detection:**
- Error logged at ERROR level with status code
- Retry count exceeded (3 attempts with exponential backoff)

**Recovery Steps:**
1. Wait for GitHub to recover (check status.github.com)
2. For urgent diffs, download manually and use `ParseDiffInput` with local content
3. If outage exceeds 30 min, activate manual review process

**Prevention:**
- No caching (GitHub API is the source of truth)
- Retry with exponential backoff built in
- Rate limit tracking via response headers

### Scenario 2: Corrupted Diff Data

**Impact:** Parsing produces partial or incorrect results.

**Symptoms:**
- `DiffAnalyzerError::DiffParseError`
- High error count in `DiffParseResult.errors`
- Suspicious file count (0 files for a known non-empty PR)

**Detection:**
- Check `DiffParseResult.files_parsed` vs. `DiffParseResult.files_failed`
- Log raw diff at DEBUG level before parsing
- Monitor `files_parsed == 0` for non-empty PRs

**Recovery Steps:**
1. Check if the diff source is valid (re-fetch from GitHub API)
2. Validate raw diff format before parsing
3. If corruption is persistent, escalate to platform team

**Prevention:**
- Validate diff format before parsing (must start with `diff --git`)
- Return partial results with error list
- Log forensic data at DEBUG level

### Scenario 3: Resource Exhaustion

**Impact:** Processing times out or consumes excessive memory.

**Symptoms:**
- Diff parsing takes > 60 seconds
- Memory usage spikes (from storing large raw diffs)
- OOM kills

**Detection:**
- Monitor processing time per file
- Check `PrDiff.total_size_bytes` against configured limit
- Watch for limits_exceeded flag

**Recovery Steps:**
1. `PolicyLimits` should prevent this via `max_diff_size` (default 10 MB)
2. If triggered, progressive degradation keeps files within limits
3. Adjust `PolicyLimits` for the specific repository if needed

**Prevention:**
- Progressive degradation is always enabled
- Per-file limits prevent single-file blowups
- Total size limit prevents multi-file blowups

### Scenario 4: Configuration Error

**Impact:** Module behaves incorrectly due to bad configuration.

**Symptoms:**
- `DiffAnalyzerError::InvalidPolicyLimits`
- Risk classification assigns wrong levels
- All diffs rejected or all passed

**Detection:**
- Validate `PolicyLimits` at initialization
- Log the effective configuration at INFO level on startup

**Recovery Steps:**
1. Reset to `PolicyLimits::default()` (10 MB, 100 files, 5000 lines)
2. For custom patterns, validate each pattern is a valid glob
3. Incrementally adjust from defaults

**Prevention:**
- `PolicyLimits::default()` is always safe
- Factory interface validates parameters
- Configuration logged at startup

## Backup Strategy

The module has **no persistent state** — each invocation is independent.
No backups are needed.

### What to Preserve

| Artifact | Retention | Location |
|----------|-----------|----------|
| Source code | Git history | `actions/src/diff_analyzer/` |
| Architecture docs | Git history | `actions/.pi/architecture/modules/diff-analyzer.md` |
| Issue specs | Git history | `actions/.pi/issues/issue-*.md` |
| CI scripts | Git history | `actions/.pi/scripts/ci/check_diff-analyzer_*.sh` |

## Restore Procedure

Since the module is stateless:

1. **Code restore**: Checkout from Git
   ```bash
   git checkout <known-good-commit> -- actions/src/diff_analyzer/
   ```
2. **Configuration restore**: Use `PolicyLimits::default()`
3. **Verify**: Run the full test suite
   ```bash
   cargo test -p rigorix-actions -- diff_analyzer
   ```
4. **Integration check**: Run proofing scripts
   ```bash
   bash .pi/scripts/ci/stage_diff-analyzer_proofing.sh
   ```
5. **Architecture validation**: Confirm canonical references
   ```bash
   bash .pi/scripts/validate-architecture.sh
   ```

## Failover Plan

There is no active failover for the diff-analyzer module — it runs as part of
the GitHub Action runtime. Each invocation is independent.

### If New Deployments Fail

1. Rollback to last known-good commit
2. Run proofing scripts to validate
3. Contact platform team via incident process

### On Call Process

1. **Priority**: P2 (moderate) for individual diff analysis failures
2. **Priority**: P1 (critical) for complete diff analysis outage
3. **Escalation**: GitHub issue with `diff-analyzer` label

## Testing the DR Plan

The DR plan is validated by the proofing scripts:

```bash
# Run all validation scripts
bash .pi/scripts/ci/stage_diff-analyzer_proofing.sh

# Run architecture validator
bash .pi/scripts/validate-architecture.sh

# Full CI pipeline
bash .pi/scripts/ci/run_hardening_stages.sh
```
