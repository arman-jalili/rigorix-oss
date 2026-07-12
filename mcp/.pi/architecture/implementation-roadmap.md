# Implementation Roadmap — Rigorix MCP Gateway

## Overview

This roadmap organizes implementation into 3 phases following the dependency chain identified during architecture enrichment. Each phase produces independently testable, shippable increments.

**Total estimated effort:** 14-18 days for initial build

**Dependency graph:**

```
Phase 0: MCP Server (Foundation)
    └── Phase 1: Execution Tools | Audit Tools | Template Tools
            └── Phase 2: Enterprise Proxy (feature-gated)
```

---

## Phase 0: MCP Server Foundation (Days 1-5)

### Goal

Deliver a working MCP server that can accept connections, negotiate capabilities, list tools, and route tool calls. At the end of this phase, the server starts, accepts stdio/SSE connections, and responds to `initialize`, `tools/list`, `resources/list`, and `prompts/list` with valid MCP protocol messages.

### Modules

| Module | Items | Status |
|--------|-------|--------|
| MCP Server | Protocol types, transports, session management, tool registry, routing, resource/prompt providers | 🔲 Planned |

### Dependencies

- **None** — this is the foundation module with no upstream dependencies

### Database Migrations

- **None** — MCP Server is fully in-memory (see ADR-002)

### Implementation Items

| # | Item | Description | Est. Days | Depends On |
|---|------|-------------|-----------|------------|
| 0.1 | Core Protocol Types | Implement `JsonRpcMessage`, `ToolSchema`, `ResourceSchema`, `PromptSchema`, `ServerCapabilities` as value objects with serde Serialize/Deserialize | 1 | — |
| 0.2 | Transport Layer | Implement `McpTransport` trait, `StdioTransport` (BufReader stdin + BufWriter stdout), `SseTransport` (Axum SSE endpoint) | 1.5 | 0.1 |
| 0.3 | Session Management | Implement `Session`, `SessionManager` with lifecycle management (create, destroy, evict expired) | 1 | 0.1, 0.2 |
| 0.4 | Tool Registry & Routing | Implement `ToolRegistry` (register, unregister, find, list schemas) and `RequestRouter` (prefix-based dispatch to handlers) | 1 | 0.1, 0.3 |
| 0.5 | Resource & Prompt Providers | Implement `ResourceProvider` with `rigorix://` URI resolution, `PromptProvider` with built-in Rigorix tool guide prompts | 0.5 | 0.1 |
| 0.6 | Composition Root | Wire `McpServer` with all components at the binary crate level. Config parsing (stdio vs SSE mode, bind address) | 1 | 0.2-0.5 |

### Acceptance Criteria

- [ ] Server starts and listens on stdio or configurable SSE port
- [ ] `initialize` handshake succeeds with capability negotiation
- [ ] `tools/list` returns an empty tool list (no handlers registered yet — just the base schema)
- [ ] `resources/list` returns `rigorix://` resource templates
- [ ] `prompts/list` returns pre-crafted prompt templates
- [ ] Graceful shutdown drains active requests and closes transport
- [ ] Integration test: connect via stdio, send init, verify response

---

## Phase 1: Core Tool Handlers — Execution, Audit, Template (Days 6-12)

### Goal

Deliver all three OSS tool handler modules, making the MCP server genuinely useful: AI assistants can execute plans, validate plans, check enforcement, read/trace audits, and manage templates.

### Modules

| Module | Items | Status |
|--------|-------|--------|
| Execution Tools | EngineFacade trait & impl, ExecuteHandler, ValidatePlanHandler, CheckEnforcementHandler | 🔲 Planned |
| Audit Tools | AuditQueryService trait & impl, AuditFormatter, ReadAuditHandler, ListAuditsHandler, AuditSummaryHandler | 🔲 Planned |
| Template Tools | TemplateRepository trait & impl (filesystem), TemplateConverter, ListTemplatesHandler, GetTemplateHandler, CreateTemplateHandler, ValidateTemplateHandler | 🔲 Planned |

### Dependencies

- **MCP Server** (Phase 0) — for tool registration, request routing
- **rigorix-engine (external)** — all execution and audit operations delegate to engine via EngineFacade

### Database Migrations

- **None** — Template Tools uses filesystem (`~/.rigorix/templates/`), Audit/Execution Tools delegate to rigorix-engine (see ADR-002)

### Implementation Items

#### Execution Tools (Days 6-8)

| # | Item | Description | Est. Days | Depends On |
|---|------|-------------|-----------|------------|
| 1.1 | EngineFacade Contract | Define `EngineFacade` trait, `PlanTemplate`, `StepDefinition`, `ExecutionResult`, `ValidationResult`, `EnforcementStatus` value objects | 0.5 | — |
| 1.2 | EngineFacade Impl | Implement `EngineFacadeImpl` wrapping rigorix-engine APIs (orchestrator, enforcer, audit query). Timeout enforcement | 1 | 1.1 |
| 1.3 | ExecuteHandler | Implement `rigorix_execute` handler with input validation, timeout, result formatting (text + JSON) | 0.5 | 1.2 |
| 1.4 | ValidatePlanHandler | Implement `rigorix_validate_plan` handler delegating to engine validation | 0.5 | 1.2 |
| 1.5 | CheckEnforcementHandler | Implement `rigorix_check_enforcement` handler for budget/status queries | 0.5 | 1.2 |
| 1.6 | SSE Progress Notifications | Add streaming progress updates for long-running `rigorix_execute` (SSE only — see ADR-004) | 0.5 | 1.3 |
| 1.7 | MCP Schema Registration | Register all 3 execution tool schemas + handlers in ToolRegistry | 0.5 | 0.4, 1.3-1.5 |

#### Audit Tools (Days 8-10)

| # | Item | Description | Est. Days | Depends On |
|---|------|-------------|-----------|------------|
| 1.8 | AuditQueryService Contract | Define `AuditQueryService` trait, `AuditFilter`, `AuditSummary` value objects | 0.5 | 1.1 |
| 1.9 | AuditFormatter | Implement markdown and JSON formatting for audit envelopes, lists, and summaries | 0.5 | 1.8 |
| 1.10 | ReadAuditHandler | Implement `rigorix_read_audit` with execution ID validation and format selection | 0.5 | 1.8, 1.9 |
| 1.11 | ListAuditsHandler | Implement `rigorix_list_audits` with filter parsing (status, time range, template, limit) | 0.5 | 1.8, 1.9 |
| 1.12 | AuditSummaryHandler | Implement `rigorix_audit_summary` with time-range parsing and aggregate formatting | 0.5 | 1.8, 1.9 |
| 1.13 | MCP Schema Registration | Register all 3 audit tool schemas + handlers in ToolRegistry | 0.5 | 0.4, 1.10-1.12 |

#### Template Tools (Days 10-12)

| # | Item | Description | Est. Days | Depends On |
|---|------|-------------|-----------|------------|
| 1.14 | TemplateRepository Contract | Define `TemplateRepository` trait, `TemplateFilter`, `TemplateSummary` value objects | 0.5 | — |
| 1.15 | FilesystemTemplateRepository | Implement filesystem-backed repository: atomic writes, TOML parsing via `toml` crate, file locking via `fs2` | 1 | 1.14 |
| 1.16 | TemplateConverter | Implement TOML↔JSON conversion, template schema validation | 0.5 | 1.14 |
| 1.17 | ListTemplatesHandler | Implement `rigorix_list_templates` with tag filtering and search | 0.5 | 1.15 |
| 1.18 | GetTemplateHandler | Implement `rigorix_get_template` with format selection (JSON/TOML) | 0.5 | 1.15 |
| 1.19 | CreateTemplateHandler | Implement `rigorix_create_template` with name validation, overwrite guard, atomic write | 0.5 | 1.15, 1.16 |
| 1.20 | ValidateTemplateHandler | Implement `rigorix_validate_template` with schema check + enforcement policy delegation | 0.5 | 1.16, 1.2 |
| 1.21 | MCP Schema Registration | Register all 4 template tool schemas + handlers in ToolRegistry | 0.5 | 0.4, 1.17-1.20 |
| 1.22 | Binary Integration Wiring | Wire all 3 handler modules into the binary composition root alongside MCP Server | 0.5 | 0.6, 1.7, 1.13, 1.21 |

### Acceptance Criteria

- [ ] `rigorix_execute` accepts a plan, delegates to engine, returns structured result with `execution_id` and `audit_uri`
- [ ] `rigorix_validate_plan` validates a plan against enforcement policies
- [ ] `rigorix_check_enforcement` returns current budget and circuit breaker status
- [ ] `rigorix_read_audit` returns audit trail by execution ID in text and JSON formats
- [ ] `rigorix_list_audits` lists recent executions with filtering
- [ ] `rigorix_audit_summary` returns aggregate statistics over a time range
- [ ] `rigorix_list_templates` discovers TOML templates from `.rigorix/templates/`
- [ ] `rigorix_get_template` reads a specific template by name
- [ ] `rigorix_create_template` saves a new template atomically (temp-file + rename)
- [ ] `rigorix_validate_template` validates template structure against schema
- [ ] All 10 OSS tools visible via `tools/list`
- [ ] Integration test: end-to-end plan execution → audit query cycle

---

## Phase 2: Enterprise Proxy (Days 13-16)

### Goal

Deliver conditional enterprise proxy that dynamically discovers and proxies `rigorix_enterprise_*` tool calls. At the end of this phase, users with enterprise credentials get upgraded tool lists with enterprise capabilities; users without see no change.

### Modules

| Module | Items | Status |
|--------|-------|--------|
| Enterprise Proxy | ProxyConfig, Secret, ProxyClient, SchemaCache, EnterpriseProxyImpl, conditional integration | 🔲 Planned |

### Dependencies

- **MCP Server** (Phase 0) — for tool registration
- **No dependency on Phase 1** — Enterprise Proxy is standalone

### Database Migrations

- **None** — Enterprise Proxy uses in-memory cache only (see ADR-002)

### Implementation Items

| # | Item | Description | Est. Days | Depends On |
|---|------|-------------|-----------|------------|
| 2.1 | ProxyConfig & Secret | Define `ProxyConfig` value object, implement `Secret<T>` wrapper with redacted Debug/Display/Serialize | 0.5 | — |
| 2.2 | JSON-RPC Types | Define `JsonRpcRequest`, `JsonRpcResponse` value objects with serde | 0.5 | — |
| 2.3 | ProxyClient | Implement HTTP client using `reqwest` with Bearer token auth, TLS verification (configurable), timeout handling, retry logic | 1.5 | 2.1, 2.2 |
| 2.4 | SchemaCache | Implement in-memory schema cache with TTL-based staleness check | 0.5 | — |
| 2.5 | EnterpriseProxyImpl | Implement proxy handler: forward method+params as JSON-RPC, map errors to diagnostic messages, publish events | 1 | 2.3, 2.4 |
| 2.6 | EnterpriseProxy Trait | Define trait with `initialize()`, `handle()`, `available_tools()`, `is_enabled()`, `metadata()` | 0.5 | 2.5 |
| 2.7 | Conditional Integration | Wire into binary composition root: feature gate (`enterprise` feature flag), config-based instantiation (`Option<Arc<dyn EnterpriseProxy>>`) | 1 | 2.6, 0.4 |
| 2.8 | Error Diagnostics | Implement clear diagnostic error messages for all failure modes: API error, network error, timeout, auth failure, schema fetch failure | 0.5 | 2.5 |

### Acceptance Criteria

- [ ] Without enterprise config: zero `rigorix_enterprise_*` tools in `tools/list`
- [ ] With enterprise config: dynamic enterprise tools appear in `tools/list`
- [ ] Enterprise tool call completes successfully and returns proxied response
- [ ] Enterprise API failure returns clear diagnostic error (not a crash or generic error)
- [ ] Enterprise API key is never logged (Secret type redacted in all output)
- [ ] HTTPS enforced with TLS verification (configurable `tls_verify: false`)
- [ ] Schema cache refresh on TTL expiry
- [ ] Integration test: start with enterprise config, verify tools appear, call a tool, verify error handling
- [ ] Integration test: start without enterprise config, verify no enterprise tools

---

## Effort Estimates

| Phase | Module | Items | Est. Days | Total |
|-------|--------|-------|-----------|-------|
| Phase 0 | MCP Server | 6 items | 5 | 5 |
| Phase 1 | Execution Tools | 7 items | 3 | 8 |
| Phase 1 | Audit Tools | 6 items | 2 | 10 |
| Phase 1 | Template Tools | 9 items | 3 | 13 |
| Phase 1 | Integration Wiring | 1 item | 0.5 | 13.5 |
| Phase 2 | Enterprise Proxy | 8 items | 4 | 17.5 |

**Total estimate:** 14-18 days (factoring in buffer for dependencies, code review, testing)

## Key Milestones

| Milestone | Phase | Target Day | Deliverable |
|-----------|-------|-----------|-------------|
| M1: MCP Server MVP | Phase 0 | Day 5 | Server starts, accepts connections, lists empty tools |
| M2: Execution Tools Complete | Phase 1 | Day 8 | Plan execution, validation, enforcement checking |
| M3: Audit Tools Complete | Phase 1 | Day 10 | Audit read, list, summary |
| M4: Template Tools Complete | Phase 1 | Day 12 | Template CRUD, validation, format conversion |
| M5: OSS Feature Complete | Phase 1 | Day 13 | All 10 OSS tools functional, integration tested |
| M6: Enterprise Proxy | Phase 2 | Day 16 | Dynamic enterprise tool discovery, proxying, error handling |
| M7: Full Release | — | Day 17-18 | Documentation, hardening, release artifacts |

---

*Generated from session: d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d*
*Date: 2026-07-12*
