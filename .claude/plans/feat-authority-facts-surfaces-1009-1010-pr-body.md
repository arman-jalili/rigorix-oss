# feat: serve the frozen authority-fact method (#1010, #1009)

The SDK catalog froze `rigorix.audit.authorityFacts` (auth `session`, mcpTool
`rigorix_audit_authority_facts`) in 0.5.0. The OSS surfaces implemented a subset
of that catalog and did not carry it — so a **published, frozen contract named a
method and a tool the governed surfaces did not provide.** This closes that gap.

## Why one PR for two issues

Both issues name each other as the dependency; the measured direction is

```
server/src/backend.rs  HostBackend::dispatch
    -> catalog::find(method)                                 // #1009
    -> app_state().handle_tool_call(entry.mcp_tool, params)  // #1010
```

The server derives nothing — it routes the catalog method to the shared
`rigorix-mcp` host by `mcp_tool`, so the **host dispatch arm is what actually
serves the server method**. Landing either half alone is a red build anyway:
`server/tests/catalog_parity.rs::every_mcp_tool_maps_one_to_one_to_a_catalog_method`
asserts set equality between the catalog's `mcp_tool` values and the host's
registered descriptors.

## #1010 — the MCP tool

| file | change |
|---|---|
| `mcp/src/audit_tools/application/dto/mod.rs` | `AuthorityFactsInput` mirroring the SDK `authorityFactsParams` |
| `mcp/src/audit_tools/application/service.rs` | `AuditAuthorityFactsHandler` trait |
| `mcp/src/audit_tools/application/service_impl.rs` | impl over `Arc<dyn AuditEnvelopeRepository>` |
| `mcp/src/audit_tools/interfaces/mcp/mod.rs` | input schema + descriptor + `AUDIT_TOOL_NAMES` 3 → 4 |
| `mcp/src/host/mod.rs` | field, wiring, dispatch arm, `all_tool_descriptors` 14 → 15 |

- **One derivation, one dialect.** The projection is the engine's
  (`ProjectAuthorityFacts`); the MCP layer never re-derives.
- **`producer_id` filters envelopes before projecting**, so the engine's
  derivation signature stays free of a transport concern.
- **Whole store** (`Some(u32::MAX)`): a truncated history re-derives to different
  facts *and* a broken chain.
- **Mandated disclosure.** The description states that a bundle DISCLOSES the
  signed envelopes — the caller receives the history, not a summary (spec §4.3.3).
- The name is byte-identical to the frozen `mcpTool` and is pinned by tests here
  and transitively by the server parity tests.

### ⚠️ The correctness detail worth reviewing

`subjects_without_record` drives the `NOT_ESTABLISHED` facts, and
`verify_authority_fact_bundle` **re-derives with that list verbatim**. So passing
the requested subjects naively would produce a bundle that **self-verifies while
asserting that an established fact had no record** — a silent contradiction, not
a crash. It is computed as `subjects \ recorded`.

Confirmed by **mutation**: the naive version still passes
`a_call_returns_a_bundle_that_verifies_by_re_derivation` and is caught only by
`an_answered_subject_is_not_reported_as_without_record`. That test is the
control; the re-derivation test alone is not sufficient.

## #1009 — the server surface

- Catalog entry 19 → 20 with `AuthLevel::Session` and the exact `mcp_tool`;
  governed tools 17 → 18 (still derived from `CATALOG`, not hand-listed).
- `catalog_parity.rs`: count 20, and the test **renamed off `…nineteen…`** — a
  name that would otherwise have lied about the count.
- One extra assertion in `mcp_streamable_http.rs` that the new tool is present.

## Spec §4.3.3 reconciled (same PR, per AC7)

The spec had reserved the name **`rigorix.audit.projection`** and argued the MCP
surface should not exist *because the SDK catalog contract had to land first*.
That precondition is now **MET**, and the catalog is authoritative. §4.3.3 now:

- records the shipped name (`rigorix.audit.authorityFacts`) and states the tool
  name must **not** be renamed to match the document;
- rewrites the rationale instead of leaving it asserting a sequencing blocker
  that no longer exists;
- keeps the disclosure requirement and adds that leaving a frozen name
  unimplemented is not an available option — if the trade is rejected, the method
  must be *removed* in a new SDK release.

## Pre-existing fixes this surfaced

- **Stale tool counts** in three MCP test files (14 → 15) — exact-count
  assertions that the new tool legitimately invalidates.
- **`.pi/panic-baseline.tsv` refreshed** for a pure line shift (the 3 audit
  descriptor panics moved +27 lines, `app_state()` +37). The one genuinely new
  entry is the new descriptor's const-schema `expect`, the same pattern the other
  13 descriptors already use. `validate-guards` is green.
- **`rustls` 0.23.45 → 0.23.46** (separate commit): four new RUSTSEC advisories
  block the CI `Audit` job. Pre-existing — `git diff main -- Cargo.lock` was empty
  before the bump and the same four report against `main`. Same advisory-DB drift
  class as the #1003 anyhow/quinn-proto bump.

## Acceptance criteria

| # | #1010 criterion | Status |
|---|---|---|
| 1 | appears in `tools/list` | ✅ test |
| 2 | `AUDIT_TOOL_NAMES` = 4, counts updated | ✅ test + 3 stale counts fixed |
| 3 | `tools/call` returns a schema-conforming bundle | ✅ verifies by re-derivation |
| 4 | description discloses the envelope exposure | ✅ test |
| 5 | name byte-identical to the frozen `mcpTool` | ✅ test + parity guard |

| # | #1009 criterion | Status |
|---|---|---|
| 1 | entry with `Session` + correct `mcp_tool` | ✅ |
| 2 | method served; result matches the SDK bundle schema | ✅ parity verified field-by-field |
| 3 | auth/mcpTool match the frozen catalog | ✅ `oss_catalog_is_a_subset_of_the_sdk_catalog` |
| 4 | governed tools 17 → 18 | ✅ |
| 5 | count + test name updated, no stale "nineteen" | ✅ |
| 6 | no envelope canonical-form change | ✅ |
| 7 | spec §4.3.3 no longer contradicts the catalog | ✅ |

**Schema parity was verified field-by-field** against
`rigorix-sdk/schemas/api/authority-fact-bundle.json` before implementing:
`projection` / `anchorRef` / `authorityFact` / `evidenceRef` all match the
engine's serialization, and `AnchorRef::history_integrity` has no
`skip_serializing_if`, so the key is always present as the SDK requires. **No
engine change was needed.**

## Validation

```
cargo test --workspace
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo audit                                  # 0 vulnerabilities
bash .pi/scripts/validate-guards.sh          # + script-drift
bash .pi/scripts/local-ci.sh                 # 114/114
```

Refs #1010 · #1009
