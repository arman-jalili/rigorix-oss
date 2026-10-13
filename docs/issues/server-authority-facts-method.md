---
guardian_issue:
  id: "ISSUE-SERVER-AUTHORITY-FACTS-METHOD"
  title: "Server: add the frozen rigorix.audit.authorityFacts method to the catalog and expose its MCP tool"
  epic: "consequence gating / surfaces"
  component: "Server / Catalog"
  module: "server-catalog"
  status: planned
  priority: medium
  dependencies:
    - "rigorix-sdk 0.5.0 — the catalog method is DECLARED and frozen (landed)"
    - "ISSUE-MCP-AUTHORITY-FACTS-TOOL (companion issue; the stdio host descriptor/routing)"

  intent: |
    The SDK froze `rigorix.audit.authorityFacts` (auth `session`, mcpTool
    `rigorix_audit_authority_facts`) in the published 0.5.0 catalog. The OSS server
    implements a SUBSET of that catalog and currently does not carry the method, so
    a method that is declared, frozen and shipped is unimplemented on the governed
    server surface. Add it.

  in_scope:
    - "server/src/catalog.rs: add the CatalogEntry (19 -> 20 methods)"
    - "The RPC handler + backend wiring that produces the bundle"
    - "server/src/mcp_http.rs: expose it as a governed tool (17 -> 18)"
    - "server/tests/catalog_parity.rs: update the count assertion and rename the test"
    - "Reconcile spec 4.3.3, which currently records the opposite decision"

  out_of_scope:
    - "The stdio host descriptor and dispatch arm (ISSUE-MCP-AUTHORITY-FACTS-TOOL)"
    - "Enterprise ingestion/console (ADR-009 records a deliberate decision not to serve it there)"
    - "Changing the SDK catalog — it is frozen and already correct"
    - "Any change to the envelope canonical form (the release is additive)"

  affected_layers:
    domain:
      - "No change (engine owns the projection: rigorix_engine::audit::application::projection)"
    application:
      - "New: the server-side service call producing an AuthorityFactBundle"
    infrastructure:
      - "History read over the existing audit repository"
    api:
      - "New: one frozen method in the catalog; one governed MCP tool"

  canonical_references:
    - module: "rigorix-sdk schemas/api/catalog.json (frozen, 24 methods; the source of the contract)"
    - module: "rigorix-sdk schemas/api/authority-fact-bundle.json ($id, envelopes[] -> envelope-v2.json)"
    - module: "docs/spec/consequence-gating-v1.md#433-surface-cli-not-yet-an-mcp-tool-decision-2026-10-13"
    - module: "docs/spec/consequence-gating-v1.md#431-verification-shall-be-by-re-derivation-not-by-signature"
    - adr:
        - ".pi/architecture/decisions/ADR-016-audit-integrity-anchor.md"
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - last_sync: "2026-10-13"

  acceptance_criteria:
    - "`rigorix.audit.authorityFacts` is in server/src/catalog.rs with auth `Session` and mcp_tool `rigorix_audit_authority_facts`"
    - "The method is served: a request returns an AuthorityFactBundle matching the SDK schema"
    - "auth/mcpTool match the frozen SDK catalog exactly (the subset guard enforces this)"
    - "The MCP surface exposes it: governed tools 17 -> 18"
    - "catalog_parity count assertion and test name updated (no stale 'nineteen')"
    - "No envelope canonical-form change; existing verifiers still verify"
    - "spec 4.3.3 no longer contradicts the shipped catalog (name + decision)"
    - "validate-ci.sh / validate-tests.sh / validate-canonical.sh pass"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    ## Why this is not optional now

    The SDK catalog is the single closed `rigorix.*` namespace and it is FROZEN and
    published. Declaring a method there is the act of reserving it. Leaving the
    server without it means the published contract names a method and an MCP tool
    that the governed server does not provide — the client conformance corpus and
    the server disagree about the surface.

    The subset relation (server/tests/catalog_parity.rs) is deliberately one-way:
    "every in-code method MUST be declared in the SDK catalog … never a set/count
    equality". So this change is safe and additive in the guard's terms — but the
    method being absent is the gap this issue closes.

    ## What to change (measured 2026-10-13)

      server/src/catalog.rs        CATALOG: 19 entries, each { name, auth, mcp_tool }
      server/src/mcp_http.rs       governed_tools() filters mcp_tool.is_some() && auth != Admin
                                   -> currently 17 exposed
      server/tests/catalog_parity.rs
                                   assert_eq!(CATALOG.len(), 19) in
                                   `catalog_has_nineteen_unique_frozen_methods`
                                   -> 20, and RENAME the test (do not leave a
                                      name that lies about the count)

    Pattern to copy: the existing `rigorix.audit.summary` entry (auth + mcp_tool).

    ## Contract conflict to resolve in the same change

    Spec 4.3.3 reserves the name in advance as **`rigorix.audit.projection`**:

        "it is a governed method (AuthLevel), named `rigorix.audit.projection`"

    The frozen, published catalog says **`rigorix.audit.authorityFacts`**
    (tool `rigorix_audit_authority_facts`). The catalog is authoritative and already
    shipped; the spec text is stale. Update the spec to match the catalog — do not
    rename the catalog method, which would break a published contract.

    Spec 4.3.3 also argues the change is not justified because "the SDK catalog
    contract SHALL land first". That precondition is now SATISFIED (0.5.0), so the
    rationale must be rewritten rather than left asserting a sequencing blocker that
    no longer exists.

    ## Auth level

    `session`, matching the frozen catalog and the existing `rigorix.audit.*`
    family. Note the disclosure property: a bundle carries the `envelopes[]`, so the
    caller receives the signed history, not a summary. That is inherent to
    re-derivation (spec 4.3.1) and MUST be stated in the tool description.

    ## Reuse, do not reimplement

    The derivation already exists in the engine and is already exercised by the CLI:

      rigorix_engine::audit::application::projection::{ProjectAuthorityFacts,
                                                       verify_authority_fact_bundle}
      CLI precedent: `audit projection` / `audit verify-projection`
      (cli/src/cli_boundary/dispatch.rs), which lists the whole store
      (Some(u32::MAX)) because a bundle must carry a complete chain.

    The server must NOT derive anything of its own; it exposes the engine's
    projection. A second derivation is a second dialect.

  file_changes:
    - "modify: server/src/catalog.rs (one CatalogEntry)"
    - "modify: server/src/mcp_http.rs (governed tools 17 -> 18)"
    - "modify: server/src/rpc.rs (handler)"
    - "modify: server/tests/catalog_parity.rs (count + rename)"
    - "modify: docs/spec/consequence-gating-v1.md (4.3.3 name + decision)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-SERVER-AUTHORITY-FACTS-METHOD

## Intent

`rigorix-sdk` 0.5.0 froze `rigorix.audit.authorityFacts` in the published catalog,
with `auth: session` and `mcpTool: rigorix_audit_authority_facts`. The OSS server
implements a subset of that catalog and does not carry it.

A **published, frozen contract names a method and a tool the governed server does
not provide.** This issue closes that gap on the server surface.

## Dependencies

```
┌────────────────────────────────────────────┐
│ rigorix-sdk 0.5.0                          │  LANDED
│ catalog frozen: method + mcpTool + auth    │  24 methods
└──────────────────┬─────────────────────────┘
                   │
        ┌──────────┴───────────┐
        ▼                      ▼
┌───────────────────┐  ┌────────────────────────────┐
│ THIS ISSUE        │  │ ISSUE-MCP-AUTHORITY-...    │
│ server catalog    │  │ mcp descriptor + routing   │
│ 19 -> 20, 17 -> 18│  │ 3 -> 4 audit tools         │
└───────────────────┘  └────────────────────────────┘
```

## In Scope

- `server/src/catalog.rs`: the `CatalogEntry`.
- RPC handler + service wiring.
- `server/src/mcp_http.rs`: governed tool exposure.
- `server/tests/catalog_parity.rs`: count and test name.
- Spec §4.3.3 reconciliation.

## Out of Scope

- The stdio host descriptor/dispatch (companion issue).
- Enterprise (ADR-009 records a deliberate decision not to serve it there).
- The SDK catalog (frozen, already correct).

## Measured surface

| file | current | after |
|---|---|---|
| `server/src/catalog.rs` | 19 entries | 20 |
| `server/src/mcp_http.rs` | 17 governed tools | 18 |
| `server/tests/catalog_parity.rs` | `assert_eq!(CATALOG.len(), 19)`, test named `…nineteen…` | 20, renamed |

## Contract conflict to fix here

| source | name |
|---|---|
| spec §4.3.3 (reserved in advance) | `rigorix.audit.projection` |
| frozen published catalog (authoritative) | `rigorix.audit.authorityFacts` |

The catalog already shipped; **update the spec, not the catalog.**

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Entry present with `Session` auth + correct `mcp_tool` | `validate-canonical.sh` |
| 2 | Method served; result matches the SDK bundle schema | `validate-tests.sh` |
| 3 | auth/mcpTool match the frozen catalog exactly | `validate-ci.sh` |
| 4 | Governed tools 17 → 18 | `validate-tests.sh` |
| 5 | Count assertion + test name updated (no stale "nineteen") | `validate-ci.sh` |
| 6 | No envelope canonical change | `validate-tests.sh` |
| 7 | Spec §4.3.3 no longer contradicts the catalog | review |

## Implementation

> **Agent:** Do not rename the catalog method to match the spec — the catalog is
> published and frozen. Fix the spec. Reuse the engine's projection; a second
> derivation path would be a second dialect.

### Steps

1. Add the `CatalogEntry` to `server/src/catalog.rs` (copy `rigorix.audit.summary`'s shape)
2. Wire the handler to the engine projection (`ProjectAuthorityFacts::project`), listing the whole store
3. Expose it in `governed_tools()` and assert 18 in `mcp_http.rs`
4. Update `catalog_parity.rs`: count 20, rename the test
5. Update spec §4.3.3: name → `authorityFacts`, and rewrite the sequencing rationale (satisfied)
6. Verify the tool description states the bundle **discloses the envelopes**
7. Run validators
