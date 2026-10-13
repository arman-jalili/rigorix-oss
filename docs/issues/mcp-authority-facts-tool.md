---
guardian_issue:
  id: "ISSUE-MCP-AUTHORITY-FACTS-TOOL"
  title: "MCP: implement the frozen rigorix_audit_authority_facts tool (descriptor + host routing)"
  epic: "consequence gating / surfaces"
  component: "MCP / Audit Tools"
  module: "audit-tools"
  status: planned
  priority: medium
  dependencies:
    - "rigorix-sdk 0.5.0 — mcpTool `rigorix_audit_authority_facts` is DECLARED and frozen (landed)"
    - "ISSUE-SERVER-AUTHORITY-FACTS-METHOD (companion issue; the server-side catalog entry)"

  intent: |
    The frozen SDK catalog names the MCP tool `rigorix_audit_authority_facts` for
    `rigorix.audit.authorityFacts`. The stdio host does not implement it, so the
    declared tool is missing from `tools/list` and unhandled in `tools/call`.
    Implement it as a fourth audit tool, alongside the existing three.

  in_scope:
    - "mcp/src/audit_tools/interfaces/mcp/mod.rs: the tool descriptor + input schema"
    - "AUDIT_TOOL_NAMES: 3 -> 4 entries"
    - "mcp/src/host/mod.rs: the tools/call dispatch arm"
    - "The mandated disclosure in the tool description (see below)"
    - "Tests: descriptor shape, name registration, and a call that produces a bundle"

  out_of_scope:
    - "The server-side catalog entry and RPC handler (companion issue)"
    - "Enterprise proxy/console (ADR-009 records a deliberate decision)"
    - "Adding a typed wrapper in the SDK clients (the catalog entry is server_only)"
    - "Changing tool names — they are frozen and already declared in the SDK"

  affected_layers:
    domain:
      - "No change (the projection is the engine's: ProjectAuthorityFacts)"
    application:
      - "New: a handler that lists history and returns the AuthorityFactBundle"
    infrastructure:
      - "Reads the audit history through the existing wiring"
    api:
      - "New: one MCP tool — descriptor, schema, dispatch; audit tools 3 -> 4"

  canonical_references:
    - module: "rigorix-sdk schemas/api/catalog.json (mcpTool `rigorix_audit_authority_facts`, stability frozen)"
    - module: "rigorix-sdk schemas/api/authority-fact-bundle.json"
    - module: "docs/spec/consequence-gating-v1.md#433-surface-cli-not-yet-an-mcp-tool-decision-2026-10-13"
    - module: "mcp/src/audit_tools/interfaces/mcp/mod.rs (the existing three-tool pattern)"
    - adr:
        - ".pi/architecture/decisions/ADR-001-governed-execution-namespace.md"
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - last_sync: "2026-10-13"

  acceptance_criteria:
    - "`rigorix_audit_authority_facts` appears in `tools/list`"
    - "`AUDIT_TOOL_NAMES` carries it (3 -> 4) and any count assertion is updated"
    - "`tools/call` returns an AuthorityFactBundle conforming to the SDK schema"
    - "The descriptor's description states that the bundle DISCLOSES the signed envelopes"
    - "The tool name is byte-identical to the frozen catalog's `mcpTool` value"
    - "Tool names are not renamed (frozen catalog)"
    - "validate-ci.sh / validate-tests.sh pass"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    ## The name and auth are already fixed — do not invent them

    The frozen SDK catalog declares, for `rigorix.audit.authorityFacts`:

      "mcpTool": "rigorix_audit_authority_facts"
      "auth": "session"
      "stability": "frozen"

    The tool name is part of the published contract: it must be byte-identical, and
    it must NOT be renamed. A mismatch here is exactly what the server's parity test
    and the SDK subset guard exist to catch.

    ## Where the code goes (measured 2026-10-13)

      mcp/src/audit_tools/interfaces/mcp/mod.rs
          - input JSON Schema (follow the `rigorix_audit_summary` schema block)
          - descriptor fn returning { name, description, inputSchema }
          - pub const AUDIT_TOOL_NAMES: &[&str] = &[
                "rigorix_read_audit", "rigorix_list_audits", "rigorix_audit_summary",
            ];   -> add the fourth
      mcp/src/host/mod.rs
          - `match tool_name { "rigorix_execute" => ..., "rigorix_plan" => ..., ... }`
            -> add the arm

    ## Params

    The bundle is requested per subject, with an optional producer scope:

      { "subjects": ["..."], "producer_id": "..." }   // producer_id optional

    Mirror the SDK `authorityFactsParams` `$defs` exactly — it is the contract.

    ## Mandated disclosure (not optional)

    Spec 4.3.3 fixes this in advance, and it must be declared rather than
    discovered: a bundle carries the `envelopes[]`, i.e. **the caller receives the
    signed history, not only a summary.** That is inherent to re-derivation
    (4.3.1). The tool description MUST say so.

    ## Reuse the engine's derivation

      rigorix_engine::audit::application::projection::ProjectAuthorityFacts
      CLI precedent: cli/src/cli_boundary/dispatch.rs lists the whole store
      (Some(u32::MAX)) because a bundle must carry a complete, verifiable chain.

    Do not re-derive in the MCP layer. One derivation, one dialect.

    ## Consequence worth stating in the PR

    This makes the projection reachable over the agent-visible surface. That was the
    substance of the 4.3.3 objection ("widens the agent-visible surface for
    something fundamentally offline"). The counter is that the SDK has already
    frozen the name and the auth level, so the surface is committed; what changes
    here is only whether it is implemented. If that trade is rejected, the correct
    action is to REMOVE the method from the SDK catalog in a new SDK release —
    not to leave a frozen name unimplemented. Pick one; do not drift.

  file_changes:
    - "modify: mcp/src/audit_tools/interfaces/mcp/mod.rs (schema + descriptor + AUDIT_TOOL_NAMES)"
    - "modify: mcp/src/host/mod.rs (dispatch arm)"
    - "create: mcp/src/audit_tools/interfaces/mcp/tests or extend existing handlers tests"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-MCP-AUTHORITY-FACTS-TOOL

## Intent

The frozen SDK catalog declares the MCP tool `rigorix_audit_authority_facts`. The
stdio host exposes three audit tools and does not include it. Implement it.

## Dependencies

```
┌──────────────────────────────────────────┐
│ rigorix-sdk 0.5.0                        │  LANDED
│ mcpTool: rigorix_audit_authority_facts   │  frozen
└──────────────────┬───────────────────────┘
                   │
        ┌──────────┴───────────┐
        ▼                      ▼
┌─────────────────────┐  ┌────────────────────────┐
│ THIS ISSUE          │  │ ISSUE-SERVER-AUTHORITY-│
│ mcp descriptor +    │  │ FACTS-METHOD           │
│ host routing        │  │ server catalog 19->20  │
│ audit tools 3 -> 4  │  │ + RPC handler          │
└─────────────────────┘  └────────────────────────┘
```

## The frozen contract (do not re-invent)

| field | value |
|---|---|
| method | `rigorix.audit.authorityFacts` |
| mcpTool | `rigorix_audit_authority_facts` |
| auth | `session` |
| stability | `frozen` |
| params | `{ subjects: string[], producer_id?: string }` |
| result | `api/authority-fact-bundle.json` (`$id`) |

## Where the code goes

```rust
// mcp/src/audit_tools/interfaces/mcp/mod.rs
pub const AUDIT_TOOL_NAMES: &[&str] = &[
    "rigorix_read_audit",
    "rigorix_list_audits",
    "rigorix_audit_summary",
    "rigorix_audit_authority_facts",   // <- new
];
```

```rust
// mcp/src/host/mod.rs — the tools/call dispatch
match tool_name {
    "rigorix_execute" => { ... }
    "rigorix_plan" => { ... }
    // add the arm
}
```

## Mandated disclosure

Spec §4.3.3 fixes this in advance: the tool's description **must state that a bundle
carries the signed envelopes** — the caller receives the history, not a summary.
That is inherent to re-derivation (§4.3.1) and must be declared, not discovered.

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Tool appears in `tools/list` | `validate-tests.sh` |
| 2 | `AUDIT_TOOL_NAMES` = 4; count assertions updated | `validate-ci.sh` |
| 3 | `tools/call` returns a schema-conforming bundle | `validate-tests.sh` |
| 4 | Description discloses the envelope exposure | review |
| 5 | Name byte-identical to the frozen `mcpTool` | `validate-canonical.sh` |

## Implementation

> **Agent:** The name is frozen — copy it from the SDK catalog, never retype it from
> memory. Reuse the engine's `ProjectAuthorityFacts`; do not re-derive in the MCP
> layer.

### Steps

1. Add the input schema mirroring the SDK `authorityFactsParams` `$defs`
2. Add the descriptor (name **exactly** `rigorix_audit_authority_facts`) with the mandated disclosure
3. Extend `AUDIT_TOOL_NAMES` to four; fix any count assertion
4. Add the dispatch arm calling the engine projection over the whole store
5. Tests: descriptor shape, listing, and a call returning a verifiable bundle
6. Run validators
