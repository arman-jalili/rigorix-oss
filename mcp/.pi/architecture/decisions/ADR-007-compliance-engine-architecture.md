# ADR-007: Compliance Engine Architecture — Read-Only Audit Bridge

**Status:** Implemented (read-only audit bridge: ReadAuditHandler/ListAuditsHandler in mcp/src/audit_tools/application/service_impl.rs)
**Date:** 2026-07-12
**Session:** d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d

## Context

Compliance is a core value proposition of the MCP Gateway. Per the domain exploration, key compliance requirements include:

1. **Audit immutability**: Once recorded, audit trails cannot be modified or deleted through MCP tools
2. **Read-only audit**: MCP audit tools are query-only
3. **Deterministic replay**: Every execution produces an execution ID and audit trail that can be replayed
4. **Cross-tool audit**: Enterprise aggregates audit across all AI tools from a single MCP integration
5. **Full SOC2/SOX-grade audit trails** for enterprise customers

The MCP Gateway must support these requirements without implementing any compliance logic itself — all immutable audit storage happens in rigorix-engine (OSS) and rigorix-enterprise-server (enterprise).

## Decision

1. **MCP audit tools are query-only proxies** — They never create, modify, or delete audit records. Audit mutation is strictly prohibited by the MCP protocol's tool interface.
2. **No local audit cache** — Every audit read/query goes directly to rigorix-engine. This ensures the data is always current and never stale.
3. **Audit data format**: The gateway supports two output formats:
   - **Text/markdown**: Human-readable for AI assistant chat consumption
   - **Structured JSON**: Machine-readable for programmatic processing
   - The `AuditFormatter` service handles format conversion; no business logic
4. **Enterprise compliance aggregation**: The enterprise proxy provides `rigorix_enterprise_*` tools for cross-team audit, approval workflows, and policy management. These are entirely proxied — the gateway stores nothing.
5. **No compliance policy enforcement in gateway**: Policy enforcement (tool allowlists, budget limits, risk thresholds) is handled by rigorix-engine. The gateway reports enforcement status but does not enforce.
6. **Non-repudiation via execution IDs**: Every `rigorix_execute` call returns an `execution_id` (UUID) and an `audit_uri` (`rigorix://audit/{execution_id}`). These are deterministic references that can be verified against the audit trail.

## Consequences

- **Positive**: Zero compliance logic in the gateway — significantly reduces audit surface for the gateway itself
- **Positive**: No mutable audit state means simpler disaster recovery (just restore engine audit store)
- **Positive**: Executions are traceable across AI tools via shared MCP server (same audit store, same IDs)
- **Positive**: Enterprise compliance features are naturally gated behind the enterprise proxy
- **Negative**: All audit queries depend on rigorix-engine availability (acceptable — engine is local)
- **Negative**: No offline audit access (acceptable — MCP server requires engine to be useful anyway)
- **Negative**: Policy enforcement cannot be overridden at the gateway level (this is a feature, not a bug)

## Alternatives Considered

| Alternative | Rationale for Rejection |
|-------------|------------------------|
| Local audit cache with periodic sync | Stale data could cause compliance gaps; adds write-mutation risk to a read-only system |
| Gateway-level policy enforcement | Duplicates engine policy logic; would create two enforcement points that could diverge |
| Immutable log append at gateway | Gateway is a protocol bridge, not a storage node; audit records belong in the engine that executed the plan |
| Client-side audit hashing (e.g., send audit hash to client for verification) | Complexity without clear value; rigorix-engine already provides HMAC integrity via AuditEnvelope |

## Affected Modules

- Audit Tools (read-only handlers, AuditFormatter)
- Execution Tools (execution_id generation, audit_uri return)
- Enterprise Proxy (enterprise compliance tools)
