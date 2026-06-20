# Action Input Disaster Recovery Plan

## Scope

This DR plan covers the Action Input module within the `rigorix-actions` crate.
The module is stateless and read-only — it parses environment variables and files,
producing typed Rust structs. There is no database, no persistent storage, and no
external service dependencies.

## RTO / RPO

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Module is stateless; rebuild is fast |
| RPO (Recovery Point Objective) | N/A | No persistent state to recover |

## Backup Strategy

Not applicable. The Action Input module has no state to back up:
- No database tables
- No cache files
- No queue messages
- No user data

## Failure Scenarios

### Scenario 1: Corrupted `action.yml`

**Symptoms:**
- `ActionYmlParseError` at startup
- Config loading fails silently, falls back to defaults

**Impact:**
- Workflow inputs lose YAML defaults (env overrides still work)

**Recovery:**
```bash
# 1. Validate YAML syntax
yamllint action.yml

# 2. Check inputs section structure
grep -A 5 "inputs:" action.yml

# 3. Fix any syntax errors and redeploy
```

### Scenario 2: Missing Environment Variables

**Symptoms:**
- `MissingRequiredInput` errors
- Default values used instead of expected inputs

**Impact:**
- Workflow behaves differently than configured
- Intent may be missing → mode falls to `status`

**Recovery:**
```bash
# 1. Check that inputs are passed in workflow YAML
grep -A 20 "with:" .github/workflows/*.yml

# 2. Verify env var naming: INPUT_<NAME> (uppercase, hyphens→underscores)
printenv | grep "^INPUT_"

# 3. Check for typos in input names
```

### Scenario 3: Corrupted Event Payload

**Symptoms:**
- `EventPayloadParseError` at startup
- GitHub event fields missing or wrong type

**Impact:**
- Cannot determine event context (PR, issue, push)
- Comment commands not detected

**Recovery:**
```bash
# 1. Inspect the raw event payload
cat $GITHUB_EVENT_PATH | python -m json.tool

# 2. Verify GITHUB_EVENT_NAME is set correctly
echo $GITHUB_EVENT_NAME

# 3. Redeploy with debug logging to capture event shape
RUST_LOG=debug rigorix-action
```

## Failover Plan

Not applicable. The Action Input module is a single-instance, single-process
module with no distributed system dependencies. There is no failover target.

## Testing the DR Plan

Run the proofing scripts to verify module health:

```bash
# 1. Verify all contracts implemented
bash actions/.pi/scripts/ci/check_action-input_contracts.sh

# 2. Verify tests pass and builds succeed
bash actions/.pi/scripts/ci/check_action-input_coverage.sh

# 3. Full CI hardening run
bash actions/.pi/scripts/ci/run_hardening_stages.sh --stages action-input_proofing
```

## Recovery Contact

For DR issues, contact the Rigorix engineering team via:
- **GitHub Issues**: https://github.com/arman-jalili/rigorix-oss/issues
- **Incident Response**: File issue with label `incident` and `action-input`
