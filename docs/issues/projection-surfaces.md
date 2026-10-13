---
guardian_issue:
  id: "ISSUE-PROJECTION-SURFACES"
  title: "Authority-fact projection surfaces: CLI command + MCP tool (the MCP tool is a frozen-catalog change across three repos)"
  epic: "consequence gating"
  component: "Audit / Evidence"
  module: "audit-integrity"
  status: planned
  priority: medium
  dependencies:
    - "ISSUE-AUTHORITY-FACT-PROJECTION (#1001) — domain + re-derivation verification, LANDED"
    - "rigorix-sdk: catalog method (filed separately)"
    - "rigorix-enterprise: ingestion + console (filed separately)"

  intent: |
    Make the authority-fact projection reachable: a CLI command to emit a bundle,
    and an MCP tool so it is available over the governed surface. The two halves
    have very different costs and must not be conflated — the CLI is our own
    surface (contained), while the MCP tool changes the FROZEN catalog and is
    therefore a cross-repo contract change.

  in_scope:
    - "CLI: a command that emits an AuthorityFactBundle (projection + history + subjects)"
    - "CLI: a verify subcommand applying the re-derivation check, so the check is runnable"
    - "Decision + design for the MCP tool name, schema and auth level (frozen catalog)"
    - "If the tool is added: mcp descriptor + host routing + server catalog parity + conformance"

  out_of_scope:
    - "The projection domain/application code and the re-derivation verifier (landed in #1001)"
    - "The SDK catalog edit itself (needs its own issue and release; filed in rigorix-sdk)"
    - "Enterprise console rendering (filed in rigorix-enterprise)"
    - "A signed projection (documented as deferred in spec 4.3.2)"

  affected_layers:
    domain:
      - "No change (landed in #1001)"
    application:
      - "Expose ProjectAuthorityFacts + verify_authority_fact_bundle through a CLI-facing service if not already reachable"
    infrastructure:
      - "History read for the bundle (reuse the anchored history reader)"
    api:
      - "New: CLI subcommand(s)"
      - "New (decision required): MCP tool descriptor + routing — a FROZEN catalog change"

  canonical_references:
    - module: "docs/spec/consequence-gating-v1.md#431-verification-shall-be-by-re-derivation-not-by-signature"
    - module: "docs/spec/consequence-gating-v1.md#432-a-signed-projection-is-a-permitted-optimisation-with-a-condition"
    - adr:
        - ".pi/architecture/decisions/ADR-016-audit-integrity-anchor.md"
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - last_sync: "2026-10-12"

  acceptance_criteria:
    - "A CLI command emits a bundle that a third party can verify by re-derivation"
    - "A CLI verify subcommand applies the re-derivation check and exits non-zero on failure"
    - "The output carries no absolute path, username or secret (spec NC + ADR-017 redaction)"
    - "If an MCP tool is added: the name is added to the SDK catalog contract FIRST, then both servers, then conformance"
    - "Catalog parity tests (OSS server + SDK conformance) are updated in the same change set"
    - "validate-ci.sh / validate-tests.sh / validate-canonical.sh pass"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    ## Do the CLI first; treat the MCP tool as a separate, gated decision

    The CLI is our own surface and blocks nothing. The MCP tool is a frozen-catalog
    change with this actual shape (measured 2026-10-12):

      SDK  schemas/api/catalog.json                      -> 23 methods (canonical)
           python/rigorix_verifier/_schemas/api/          -> 23 (copy)
           typescript/_schemas/api/                       -> 23 (copy)
      OSS  server/src/mcp_http.rs:529                     -> assert_eq!(exposed.len(), 17)
           server/tests/catalog_parity.rs                -> names == CATALOG.len()

    So adding one method means, in order:

      1. rigorix-sdk  — the canonical catalog + both polyglot copies + conformance
                        fixtures (a minor SDK release; verifiers must accept the
                        new method set)
      2. rigorix-oss  — mcp descriptor + routing, then server parity/conformance
      3. enterprise   — ingestion/console if it surfaces the tool

    That sequencing is not optional: a verifier built against the old catalog will
    reject or ignore the new method, exactly as the 0.3.0 -> 0.4.0 attribution
    sequencing bit us before.

    ## Is a tool the right surface at all?

    Worth deciding explicitly rather than by default. The projection is an EXPORT
    for an external determination consumer or an auditor, not a step an agent takes
    mid-run. Arguments for the tool: reachability over the governed surface,
    consistent auth (`AuthLevel`), and it reuses the existing audit tool plumbing.
    Arguments against: it widens the agent-visible surface for something that is
    fundamentally an offline artifact, and it costs a three-repo contract change.

    If it is added, note the disclosure property in the tool description: a bundle
    carries the ENVELOPES, so a caller receives the history, not just a summary.
    That is inherent to the re-derivation design (spec 4.3.1) and should be stated,
    not discovered.

    ## Redaction

    Reuse the ADR-017 redaction helpers. The projection is explicitly shared with
    auditors, so an absolute path or username leak is a real disclosure. The
    existing test `no_absolute_paths_or_username_in_the_canonical_bytes` covers the
    projection; the CLI must not add a leak in its own output (e.g. echoing the
    store path).

  file_changes:
    - "modify: cli/src/cli_boundary/cli.rs (new subcommand)"
    - "modify: cli/src/cli_boundary/orchestrator.rs (dispatch)"
    - "create: cli/src/cli_boundary/tests (bundle emit + verify subcommand)"
    - "modify (only if the tool is approved): mcp/src/audit_tools/interfaces/mcp/mod.rs"
    - "modify (only if the tool is approved): mcp/src/host/mod.rs (routing)"
    - "modify (only if the tool is approved): server/tests/catalog_parity.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-PROJECTION-SURFACES — make the projection reachable

## Intent

`#1001` landed the projection domain, the bundle and the re-derivation verifier,
but nothing can reach them:

```
grep -rln 'ProjectAuthorityFacts|AuthorityFactBundle' cli/src mcp/src
  → (empty)
```

This issue makes them reachable. The CLI half is contained; the MCP half is a
three-repo contract change and is called out as such rather than bundled in.

## Dependencies

```
┌──────────────────────────────────────┐
│ ISSUE-AUTHORITY-FACT-PROJECTION      │  LANDED (d9e4b7c3)
│ domain + bundle + re-derivation      │
└───────────────┬──────────────────────┘
                │
                ▼
┌──────────────────────────────────────┐
│ ISSUE-PROJECTION-SURFACES            │ ← THIS ISSUE
│ CLI (contained) · MCP (frozen)       │
└───────────────┬──────────────────────┘
                │ MCP half requires:
                ▼
┌──────────────────────────────────────┐
│ rigorix-sdk: catalog method          │  (filed separately)
│ rigorix-enterprise: ingest + console │  (filed separately)
└──────────────────────────────────────┘
```

## In Scope

- CLI command emitting a bundle.
- CLI verify subcommand applying the re-derivation check.
- The MCP tool decision **and** design (name, schema, auth level).

## Out of Scope

- The projection domain/application and verifier (landed).
- The SDK catalog edit (separate repo and release).
- Enterprise console rendering (separate repo).
- A signed projection (spec 4.3.2, deferred with conditions).

## The frozen-catalog cost (measured)

| repo | artifact | current |
|---|---|---|
| SDK | `schemas/api/catalog.json` (canonical) | 23 methods |
| SDK | `python/…/_schemas/api/catalog.json`, `typescript/_schemas/api/` | 23 (copies) |
| OSS | `server/src/mcp_http.rs:529` | `assert_eq!(exposed.len(), 17)` |
| OSS | `server/tests/catalog_parity.rs` | `names == CATALOG.len()` |

Ordering is load-bearing: **SDK catalog first**, then both servers, then
conformance — a verifier built against the old catalog will not accept the new
method, which is exactly how the 0.3.0→0.4.0 attribution sequencing went wrong.

## Question worth answering explicitly

Is an MCP **tool** the right surface for this? The projection is an export for an
external consumer or auditor, not a step an agent takes mid-run. It reuses the
governed surface and auth, but it widens the agent-visible surface for something
fundamentally offline, and it costs a three-repo contract change. The CLI may be
the whole answer.

If the tool is added, its description must state that a bundle carries the
**envelopes** — the caller receives the history, not a summary. That is inherent to
re-derivation (spec 4.3.1) and should be declared, not discovered.

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | CLI emits a bundle verifiable by re-derivation | `validate-tests.sh` |
| 2 | CLI verify exits non-zero on a failed check | `validate-tests.sh` |
| 3 | No absolute path / username / secret in CLI output | `validate-security.sh` |
| 4 | If MCP: SDK catalog updated first, then OSS, then conformance | `validate-canonical.sh` |
| 5 | If MCP: catalog parity + conformance updated in one change set | `validate-ci.sh` |

## Implementation

> **Agent:** Implement the CLI first and land it. Do not start the MCP tool until
> the SDK catalog method exists — the parity tests will fail otherwise, and that
> is by design.

### Steps

1. Add the CLI subcommand to `cli.rs` and dispatch it in `orchestrator.rs`
2. Emit the bundle (projection + envelopes + subjects) via the existing serializer
3. Add a verify subcommand calling `verify_authority_fact_bundle`, non-zero on error
4. Assert redaction in the CLI output (reuse the ADR-017 helpers)
5. Decide the MCP question; if yes, design name/schema/auth level and file the SDK dependency
6. Only after the SDK catalog lands: mcp descriptor + routing + parity
7. Run all validators
