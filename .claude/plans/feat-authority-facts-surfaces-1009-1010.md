# Plan — authority-facts surfaces (#1010, #1009)

Branch `feat/authority-facts-surfaces-1009-1010`. Order: **#1010 → #1009**
(measured; see `issue-groups.md`).

## Contract (frozen — copy from the SDK, never retype)

`rigorix-sdk` 0.5.0 `schemas/api/catalog.json`:

| field | value |
|---|---|
| method | `rigorix.audit.authorityFacts` |
| mcpTool | `rigorix_audit_authority_facts` |
| auth | `session` |
| stability | `frozen` |
| params | `{ subjects: string[] (minItems 1, required), producer_id?: string }`, `additionalProperties: false` |
| result | `$ref https://rigorix.dev/schemas/api/authority-fact-bundle-v1.json` |

## Verified schema parity (engine serialization ↔ SDK bundle schema)

| SDK def | required | engine type | verdict |
|---|---|---|---|
| `projection` | spec, version, generated_at, claim_ceiling, facts | `AuthorityFactProjection` | ✅ names match |
| `anchorRef` | head_sequence, **history_integrity** | `AnchorRef` (`Option<String>`, no `skip_serializing_if` → emits `null`) | ✅ key always present |
| `authorityFact` | subject, scope, established_at, epistemic_status, chain_position | `AuthorityFact` | ✅ |
| `evidenceRef` | kind, reference | `EvidenceRef` | ✅ |
| bundle | projection, envelopes | `AuthorityFactBundle` | ✅ (`subjects_without_record` optional) |

`EpistemicStatus` is `SCREAMING_SNAKE_CASE` → `ESTABLISHED` / `NOT_ESTABLISHED` ✅.

**Conclusion: no engine change is needed.** The server/MCP layers expose the
engine's projection verbatim (one derivation, one dialect).

## Design decisions

1. **#1010 is the real server implementation.** `HostBackend::dispatch` routes a
   catalog method to `app_state().handle_tool_call(entry.mcp_tool, params)`. So
   the server serves `authorityFacts` *through* this MCP tool. #1009 adds only the
   catalog entry + governed exposure.
2. **`producer_id` is applied by filtering envelopes before projecting.**
   `ProjectAuthorityFacts::project` has no producer parameter; inventing one would
   change the engine's derivation surface for a transport concern.
3. **`subjects_without_record` MUST be computed, not copied from `subjects`.**
   `project()` emits a `NOT_ESTABLISHED` fact per entry; the verifier re-derives
   with it verbatim, so a wrong value still *self-verifies* while asserting that
   an established subject had no record — a silent contradiction. Compute
   `subjects \ facts.map(|f| f.subject)`.
4. **Reuse the shared repository.** `init_host` already builds a
   `LocalAuditEnvelopeRepository` for `.rigorix/audit`; hoist it into an `Arc` and
   share it with the new handler rather than opening a second reader.
5. **Whole store.** A bundle must carry a complete, verifiable chain → `list(..,
   Some(u32::MAX))`, matching the CLI precedent.
6. **Spec §4.3.3 is corrected, not deleted**: the name becomes
   `rigorix.audit.authorityFacts`, and the "not justified" rationale is rewritten
   because its precondition (SDK-first) is now satisfied.

## #1010 — MCP tool (first)

| file | change |
|---|---|
| `mcp/src/audit_tools/application/dto/mod.rs` | `AuthorityFactsInput` |
| `mcp/src/audit_tools/application/service.rs` | `AuditAuthorityFactsHandler` trait |
| `mcp/src/audit_tools/application/service_impl.rs` | impl over `Arc<dyn AuditEnvelopeRepository>` |
| `mcp/src/audit_tools/application/mod.rs` | re-exports |
| `mcp/src/audit_tools/interfaces/mcp/mod.rs` | input schema + descriptor + `AUDIT_TOOL_NAMES` 3→4 |
| `mcp/src/host/mod.rs` | handler field + wiring + dispatch arm + descriptor in `all_tool_descriptors` |
| `mcp/tests/schema_descriptor_tests.rs` | count 14→15 |
| `mcp/tests/authority_facts_tool_test.rs` | new |

## #1009 — server catalog (second)

| file | change |
|---|---|
| `server/src/catalog.rs` | `CatalogEntry` 19→20 |
| `server/src/mcp_http.rs` | governed tools 17→18 (+ assertion) |
| `server/tests/catalog_parity.rs` | 19→20, rename `catalog_has_nineteen_unique_frozen_methods` |
| `docs/spec/consequence-gating-v1.md` | §4.3.3 name + rationale |

## TDD cases (red first)

1. descriptor name is byte-identical to the frozen catalog `mcpTool`
2. descriptor description discloses the envelope exposure
3. `AUDIT_TOOL_NAMES` has 4 entries and includes the tool
4. `all_tool_descriptors()` count 14→15 and includes the tool
5. a call returns a bundle that **`verify_authority_fact_bundle` accepts** (the real gate)
6. a subject **with** a record is NOT reported in `subjects_without_record`
7. `subjects: []` is rejected (schema `minItems: 1`)
8. `producer_id` restricts the projected chain
9. server: catalog entry present with `Session` + exact `mcp_tool`; 20 methods; 18 governed tools
10. server: parity assertion + test name carry no stale "nineteen"
