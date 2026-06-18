# ISSUE-ASYNC-EXEC: Async Execution + Progress Events + TUI Subscription

## Intent
Replace blocking `std::process::Command` with async `tokio::process::Command` in the execution engine, emit `NodeStarted`/`NodeCompleted` progress events during DAG execution, and wire the TUI's EventBridge to subscribe to the engine's event bus for real-time updates.

## Scope
**Complex** — 3 bounded contexts, multi-crate (engine + cli)

## In Scope
1. Replace `std::process::Command` → `tokio::process::Command` in `service_impl.rs` (4 methods)
2. Add `EventBusService` reference to `ParallelExecutionServiceImpl` for event emission
3. Add `subscribe_receiver()` method to `EventBusService` trait
4. Emit `ExecutionEvent::NodeStarted` / `NodeCompleted` during `execute_graph()`
5. Wire TUI `EventBridge` to subscribe to the engine event bus
6. Implement `event_to_mutation()` mapping in TUI
7. Expose event bus from CLI orchestrator wiring for TUI access
8. Update factory (`ParallelExecutionFactoryImpl`) to accept event bus

## Out of Scope
- Other `ExecutionEvent` variants (NodeFailed, NodeRetrying, etc.) — these remain un-published
- Git tool test helpers using `std::process::Command` (test infra, not prod execution)
- Parallel execution (JoinSet) — current sequential dispatch loop is sufficient

## Files Changed

### Engine Crate (`engine/src/`)

| File | Change |
|------|--------|
| `execution_engine/application/service_impl.rs` | Add `event_bus` field, emit events during `execute_graph()`, make `exec_*` methods async with `tokio::process::Command` |
| `execution_engine/application/service_impl.rs` | Call `notify_progress()` from `execute_graph()` dispatch loop |
| `execution_engine/application/factory_impl.rs` | Accept `Arc<dyn EventBusService>` in factory |
| `execution_engine/application/factory.rs` | Add `event_bus` to `ParallelExecutionFactoryConfig` |
| `event_system/application/service.rs` | Add `subscribe_receiver()` to `EventBusService` trait |
| `event_system/application/event_bus_service_impl.rs` | Implement `subscribe_receiver()` |
| `event_system/application/dto/mod.rs` | Update SubscribeOutput docs |
| `orchestrator/application/builder_impl.rs` | Pass event bus to execution factory |

### CLI Crate (`cli/src/`)

| File | Change |
|------|--------|
| `tui/event_bridge.rs` | Add concrete `EventBridgeImpl` that subscribes and maps events |
| `tui/event_bridge.rs` | Implement `event_to_mutation()` for all 11 variants |
| `tui/event_bridge.rs` | Add `start()`/`stop()` lifecycle with background task |
| `tui/event_bridge.rs` | Wire ViewModel mutation application |
| `tui/event_bridge.rs` | Implement reverse command channel |
| `cli_boundary/orchestrator.rs` | Pass event bus reference when creating orchestrator |

## Acceptance Criteria
- `cargo build` passes in both engine and cli crates
- All existing tests pass (engine: 800+ tests)
- `tokio::process::Command` used instead of `std::process::Command` in prod execution
- `NodeStarted`/`NodeCompleted` events appear in persisted event log after `execute_graph()`
- TUI EventBridge successfully subscribes and receives events
- `validate-ci.sh` passes
- `validate-canonical.sh` passes
