# Runbook: execution-tools

## Overview

The execution-tools module bridges MCP tool calls to the rigorix-engine. It provides
three MCP tools: `rigorix_execute`, `rigorix_validate_plan`, and `rigorix_check_enforcement`.

**Module:** `src/execution_tools/`  
**Architecture:** Clean Architecture (DDD)  
**Package:** `rigorix-mcp`  
**Dependencies:** `rigorix-engine`

## Startup

### Prerequisites
- rigorix-engine must be initialized before EngineFacadeImpl is created
- ExecutionEnforcer must be configured with budget policies

### Initialization Order

```rust
// 1. Create engine dependencies
let orchestrator = Arc::new(OrchestratorServiceImpl::new(...));
let enforcer = Arc::new(ExecutionEnforcerImpl::new(...));
let repository = Arc::new(InMemoryExecutionRepository::new());

// 2. Create EngineFacade
let facade = EngineFacadeImpl::new(
    orchestrator,
    enforcer,
    repository,
    EngineFacadeConfig::default(),
);

// 3. Create handlers
let handlers = create_handler_instances(
    Arc::new(facade),
    Duration::from_secs(300),
);
```

### Configuration Reference

| Setting | Default | Description |
|---------|---------|-------------|
| `execute_timeout` | 300s | Max duration for plan execution |
| `validate_timeout` | 60s | Max duration for plan validation |
| `enforcement_enabled` | true | Whether to check enforcement pre-execution |
| `repo_root` | `"."` | Repository root for engine operations |

## Graceful Shutdown

1. Cancel any in-flight `rigorix_execute` calls via `CancellationService`
2. Drain pending `save_execution` operations
3. Drop `EngineFacadeImpl` reference (all pending futures complete or are cancelled)

## Common Failure Modes

| Symptom | Cause | Recovery |
|---------|-------|----------|
| `EngineNotAvailable` | rigorix-engine not initialized | Initialize engine first |
| `Timeout` | Plan execution exceeds timeout | Increase `execute_timeout` or simplify plan |
| `BudgetExceeded` | Enforcement budget exhausted | Reset budget or switch enforcement preset |
| `ExecutionNotFound` | Invalid execution ID queried | Verify execution ID was returned by execute() |

## Monitoring

- **Logging:** Structured logs via `tracing` crate with `execution_id` span
- **Events:** `ExecutionToolsEvent` emitted for each operation lifecycle
- **Audit:** Execution results persisted in `ExecutionRepository`

## Health Check

The module exposes no direct health endpoint. Health is provided by the MCP Server's
health check which validates all registered tool handlers are responsive.
