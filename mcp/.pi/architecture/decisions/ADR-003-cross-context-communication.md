# ADR-003: Cross-Context Communication — Trait-Based DI + EventBus

**Status:** Implemented (trait-based DI throughout mcp/src; engine EventBusService wired at mcp/src/main.rs:795-797)
**Date:** 2026-07-12
**Session:** d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d

## Context

The five bounded contexts need to communicate. The communication patterns are:

| Pattern | Example | Frequency |
|---------|---------|-----------|
| **Direct method call** (sync) | MCP Server → RequestRouter resolves tool name → dispatches to ExecuteHandler | Per tool call |
| **Shared interface** (trait) | ExecuteHandler calls EngineFacade trait methods for execution/validation/enforcement | Per execution |
| **Event notification** (pub-sub) | McpSessionStarted → telemetry/logging; ToolCallReceived → audit event | Per event occurrence |
| **Conditional composition** | Enterprise Proxy registered only when enterprise feature flag is enabled | Once at startup |

Key constraints:
- MCP Server is the **entry point** — all tool calls arrive here and are routed to the appropriate handler
- Execution Tools, Audit Tools, Template Tools are **independent handlers** — they don't call each other
- Enterprise Proxy is **conditionally available** — must not be coupled at compile time

## Decision

1. **Trait-based dependency injection at composition root**: Each bounded context exposes public trait interfaces. The binary crate (`rigorix-mcp`) wires implementations at startup:

```rust
// Composition root pattern
pub struct AppContext {
    pub mcp_server: McpServer,
    pub tool_registry: ToolRegistry,
    pub engine_facade: Arc<dyn EngineFacade>,
    pub enterprise_proxy: Option<Arc<dyn EnterpriseProxy>>,
}
```

2. **In-process EventBus for notifications**: Use a simple in-memory `tokio::sync::broadcast`-based EventBus for domain events. Events are for observability (logging, metrics, telemetry) — never for operational logic.

3. **No direct crate-level dependencies between handler contexts**: Execution Tools, Audit Tools, and Template Tools each depend only on `rigorix-engine` client types (via `EngineFacade` trait). They never import each other's types.

4. **Enterprise Proxy is registered via a feature flag + dynamic check**: The binary checks for `enterprise.enabled` config at startup, instantiates the proxy if present, and injects it into `ToolRegistry`.

## Consequences

- **Positive**: Handler contexts are independently testable — mock `EngineFacade` trait
- **Positive**: Enterprise Proxy can be absent without any conditional code in handlers
- **Positive**: EventBus provides loose coupling for cross-cutting observability
- **Negative**: Composition root must know about all contexts and their trait contracts
- **Negative**: Domain events cannot carry operational logic — only observability
- **Negative**: Event order is not guaranteed with `broadcast` channel (acceptable for observability)

## Alternatives Considered

| Alternative | Rationale for Rejection |
|-------------|------------------------|
| gRPC/protobuf between contexts | Over-engineered for in-process communication; adds serialization overhead |
| Message queue (NATS/RabbitMQ) | Single-machine MCP server doesn't need distributed messaging |
| Direct struct coupling between contexts | Violates DDD bounded context isolation; would make Enterprise Proxy infection impossible |
| Shared database for inter-context state | No shared state exists across contexts; each owns its domain independently |

## Affected Modules

- MCP Server (composition root)
- Execution Tools (implements EngineFacade trait)
- Audit Tools (uses EngineFacade trait)
- Template Tools (uses EngineFacade trait)
- Enterprise Proxy (conditional registration)
