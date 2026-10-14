# Issue Groups — authority-facts surfaces

Fetched 2026-10-13 (`gh issue list --state open`). **2 open issues.**

| # | Title | Component | Priority | Labels |
|---|-------|-----------|----------|--------|
| #1009 | Server: add the frozen `rigorix.audit.authorityFacts` method to the catalog and expose its MCP tool | Server / Catalog | medium | — |
| #1010 | MCP: implement the frozen `rigorix_audit_authority_facts` tool (descriptor + host routing) | MCP / Audit Tools | medium | — |

Both were filed by `00d6f910 docs(issues): the authority-fact method must reach the server and the MCP host`.

## Grouping

| Group | Type | Issues | Branch |
|-------|------|--------|--------|
| **C** | Related batch (shared frozen contract) | #1009 + #1010 | `feat/authority-facts-surfaces-1009-1010` |

**Why one batch, not two.** They share one frozen contract
(`rigorix-sdk` 0.5.0: method `rigorix.audit.authorityFacts`, mcpTool
`rigorix_audit_authority_facts`, auth `session`) and one seam. Splitting them
would ship a catalog entry whose method is unserved, then a tool that is
unreachable — two half-states on `main` instead of one complete surface.

## Dependency order (measured, not as stated in the issues)

The issues each declare the other as a dependency. Measured against the code, the
real order is **#1010 then #1009**:

```
server/src/backend.rs  HostBackend::dispatch(method, params)
    -> catalog::find(method)              // #1009 adds the entry
    -> app_state().handle_tool_call(entry.mcp_tool, params)   // #1010 adds the arm
```

The server does **not** derive anything; it routes catalog methods to the shared
`rigorix-mcp` host by `mcp_tool`. So the MCP dispatch arm (#1010) is what actually
serves the server method (#1009). Landing #1009 alone yields a catalog entry that
returns a "not found"/error from the host.

Implementation order: **#1010 (tool + host routing) → #1009 (catalog + governed
exposure)**. Both land in one PR, so either order is safe on `main`; this order
lets each step be tested.

## Hard dependency (satisfied)

`rigorix-sdk` 0.5.0 — the catalog method is **declared and frozen** (verified:
24 methods, `stability: frozen`, `auth: session`, `mcpTool:
rigorix_audit_authority_facts`). The SDK-first precondition (D-011/D-012) is met,
which is precisely the blocker spec §4.3.3 recorded. **No SDK change is required.**

## Contract conflict to resolve in the same PR

| source | name |
|---|---|
| spec §4.3.3 (reserved in advance) | `rigorix.audit.projection` |
| frozen published catalog (authoritative) | `rigorix.audit.authorityFacts` |

The catalog is published; **fix the spec, never the catalog.** §4.3.3 also argues
the change is unjustified *because* the SDK contract must land first — that
precondition is now satisfied, so the rationale must be rewritten rather than left
asserting a blocker that no longer exists.

## Validation per issue

| Issue | Validators |
|-------|-----------|
| #1009 | ci, tests, architecture, canonical |
| #1010 | ci, tests, architecture |
