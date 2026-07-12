# DR Plan: execution-tools

## Overview

Disaster recovery plan for the execution-tools module. This module is stateless —
it delegates all execution to rigorix-engine and persists results in an in-memory
repository. The primary recovery concern is ensuring EngineFacade can reconnect
to rigorix-engine after failure.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time) | < 5 seconds | Stateless — recreate EngineFacadeImpl |
| RPO (Recovery Point) | N/A | No persistent state in execution-tools |

## Failure Scenarios

### 1. EngineFacade Connection Lost

**Symptom:** All `rigorix_execute`, `rigorix_validate_plan`, `rigorix_check_enforcement` calls return `EngineNotAvailable`

**Detection:** Consecutive failures in any handler

**Recovery:**
```bash
# 1. Check rigorix-engine status
# 2. If engine is down, restart it
# 3. Recreate EngineFacadeImpl with new engine connection
# 4. Verify: rigoris_check_enforcement returns valid status
```

**Prevention:**
- EngineFacadeImpl uses configurable timeouts
- Enforcement status is always fresh (no caching)

### 2. Enforcement Budget Exhausted

**Symptom:** `rigorix_execute` returns `BudgetExceeded`

**Detection:** Handler returns error with budget details

**Recovery:**
1. Call `rigorix_check_enforcement` to verify budget state
2. Reset budget via policy engine configuration
3. Switch to a less restrictive enforcement preset
4. Retry the execution

**Prevention:**
- Always validate plans before executing (`rigorix_validate_plan`)
- Monitor budget consumption proactively

### 3. Handler Panic/Internal Error

**Symptom:** Tool call returns internal error

**Detection:** `HandlerError::Internal` returned to MCP client

**Recovery:**
1. Check tracing logs for the `execution_id`
2. If EngineFacade is healthy, retry the call
3. If EngineFacade is unhealthy, follow scenario 1

**Prevention:**
- All handlers use `#[async_trait]` with `Send + Sync`
- No panicking code in production paths
- Comprehensive error wrapping

## Backup Strategy

No backup needed — the module is stateless. Execution results are ephemeral
(in-memory) by design. For persistent audit, rigorix-engine's audit service
handles long-term storage.

## Restoration

```bash
# 1. Ensure rigorix-engine is healthy
cargo run --bin rigorix-engine --health

# 2. Recreate execution-tools dependencies
# (See runbook Startup section)

# 3. Verify all three MCP tools respond
rigorix_check_enforcement
rigorix_validate_plan --plan minimal_plan.json
rigorix_execute --plan minimal_plan.json
```

## Post-Recovery Verification

1. `rigorix_check_enforcement` returns valid status → EngineFacade connected
2. `rigorix_validate_plan` on a valid plan returns `valid: true` → Validation works
3. `rigorix_execute` on a minimal plan returns `execution_id` → Execution works
