# Action Output Disaster Recovery Plan

## Scope

This DR plan covers the Action Output module within the `rigorix-actions` crate.
The module is stateless and write-only — it formats engine results into GitHub
Actions-native outputs. There is no database, no persistent storage, and no
external service dependencies (PR comments are optional and fire-and-forget).

## RTO / RPO

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Module is stateless; rebuild is fast |
| RPO (Recovery Point Objective) | N/A | No persistent state to recover |

## Backup Strategy

Not applicable. The Action Output module has no state to back up:
- No database tables
- No cache files
- No queue messages
- No user data
- Output files (`$GITHUB_STEP_SUMMARY`, `$GITHUB_OUTPUT`) are ephemeral per-action-run

## Failure Scenarios

### Scenario 1: `GITHUB_STEP_SUMMARY` Not Set

**Symptoms:**
- `MissingEnv("GITHUB_STEP_SUMMARY")` error
- Step summary not displayed in Actions UI

**Impact:**
- Summary markdown not rendered
- Output variables and annotations still work (they use different channels)

**Recovery:**
```bash
# 1. Verify running in GitHub Actions context
echo $GITHUB_ACTIONS

# 2. Check GITHUB_STEP_SUMMARY is set
echo $GITHUB_STEP_SUMMARY

# 3. Ensure step allows job summaries
# GitHub Actions sets this automatically — if missing, check runner version >= 2.273.0
```

### Scenario 2: Annotation Emission Fails

**Symptoms:**
- Workflow commands not parsed by Actions runner
- No annotations visible in the Actions UI

**Impact:**
- Visual feedback lost in PR/file views
- Fatal errors still fail the step (stdout exit code)

**Recovery:**
```bash
# 1. Check stdout contains valid workflow commands
# Expected format: ::error file=path,line=N::message
bash actions/.pi/scripts/ci/check_action-output_contracts.sh

# 2. Verify runner version supports workflow commands
# GitHub Actions runner v2.273.0+ required

# 3. Check for stray characters before workflow commands
# Runner requires workflow commands at line start
```

### Scenario 3: GitHub Token Missing for PR Comments

**Symptoms:**
- `MissingToken` error when trying to post PR comment
- PR comment not posted (non-fatal — output still written to summary)

**Impact:**
- Execution summary not posted as PR comment
- Step summary still available in Actions UI

**Recovery:**
```bash
# 1. Check GITHUB_TOKEN is set in workflow
grep -A 5 "GITHUB_TOKEN" .github/workflows/*.yml

# 2. Verify token permissions
# Minimum: pull-requests: write
# Set in .github/workflows YAML: permissions: pull-requests: write

# 3. If running from fork, secrets are not available
# PR comments are skipped automatically — this is expected behavior
```

### Scenario 4: Engine Returns Malformed Execution Context

**Symptoms:**
- Unexpected or garbled step summary content
- `FormatError` during summary rendering

**Impact:**
- Summary may show incorrect data (status, timing)
- Annotations and variables still correct (they use separate formatting paths)

**Recovery:**
```bash
# 1. Check engine output types
cargo test -p rigorix-engine -- execution

# 2. Verify ExecutionContext construction
cargo test --lib -p rigorix-actions -- action_output::output_formatter

# 3. Enable debug logging to inspect context
RUST_LOG=debug rigorix-action
```

### Scenario 5: Output Variable Value Too Long

**Symptoms:**
- `VariableTooLong` error
- Variable truncated or not set for downstream steps

**Impact:**
- Downstream steps may receive truncated or missing data
- Action execution continues (non-fatal)

**Recovery:**
```bash
# 1. Identify the oversized variable
# Check logs for "VariableTooLong" error with variable name

# 2. Reduce value length in caller
# GitHub Actions caps output variables at ~10KB

# 3. Consider using step summary for large content
# Step summary is not size-limited
```

## Failover Plan

Not applicable. The Action Output module is a single-instance, single-process
module with no distributed system dependencies. There is no failover target.

If output formatting fails entirely:
1. The action step fails (stdout exit non-zero)
2. GitHub Actions runner reports step failure
3. Previous step outputs (if any) remain available to downstream steps
4. Re-run the workflow to retry

## Testing the DR Plan

Run the proofing scripts to verify module health:

```bash
# 1. Verify all contracts implemented
bash actions/.pi/scripts/ci/check_action-output_contracts.sh

# 2. Verify tests pass and builds succeed
bash actions/.pi/scripts/ci/check_action-output_coverage.sh

# 3. Full CI hardening run
bash actions/.pi/scripts/ci/run_hardening_stages.sh --stages action-output_proofing
```

## Recovery Contact

For DR issues, contact the Rigorix engineering team via:
- **GitHub Issues**: https://github.com/arman-jalili/rigorix-oss/issues
- **Incident Response**: File issue with label `incident` and `action-output`
