# ADR-004: Host Contract — Typed wasm-bindgen Surface with JSON DTOs, Not REST

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

The template asks for "REST + JSON, versioning strategy". There is no server: the host
loads a `.wasm` module and calls functions in the same JS realm. A REST API would mean
inventing a transport the environment does not have — `wasm32-unknown-unknown` has no
sockets.

What the host actually needs is a narrow, versioned, hard-to-misuse contract: open a
session from a scenario plus a fixture transcript, run one attempt, export the artifact
set, verify it. Four verbs — and the fourth is three-valued.

## Decision

**One published surface: a wasm-bindgen module whose functions take and return JSON
strings (DTOs), never live Rust objects with mutable methods.**

```text
rigorix_wasm.open_session(scenario_json, fixtures_json, clock_json, seed_json) -> session_handle_json
rigorix_wasm.attempt_run(session_handle_json, action_json)                     -> run_outcome_json
rigorix_wasm.export_artifact_set(session_handle_json)                          -> artifact_set_json
rigorix_wasm.verify_artifact_set(artifact_set_json)                            -> verification_result_json
rigorix_wasm.contract_version()                                                -> semver string
```

Versioning strategy — three numbers, deliberately distinct:

| Version | Meaning | Compatibility rule |
|---------|---------|--------------------|
| npm package / crate version | Ships in the engine's release train | **Exactly equal** to `rigorix-engine` (NFR-005) |
| `contract_version` | Shape of the five signatures and DTO fields | Semver; a breaking DTO change is a major bump |
| `artifact_format_version` | Bytes the verifier reads | Stamped into every export; unknown value ⇒ `unverifiable(reason)`, never a guess (FR-010, FR-014) |

Additional rules:

1. **Inputs are validated by the engine's own types**, not a parallel schema we
   maintain. A parse failure is a typed refusal — not a `panic`, not a default.
2. **No function returns a boolean for verification.** Three-valued enum only.
3. **No function accepts a verdict, a finding, or an "approve" flag.** The only
   approval-shaped input is an approval *claim* the engine binds to the intent hash
   (FR-013).
4. `panic = "abort"` is necessary but not sufficient: every entry point converts error
   paths into `Refused` / `unverifiable`. The boundary never lets a trap cross the FFI
   line.
5. Errors carry `{ kind, message, port?, evidence_ref? }` and never a Rust backtrace, a
   host path or key material (NFR-003).
6. No `console.log` from the crate; the host logs what it receives.

## Consequences

### Positive
- JSON DTOs are language-agnostic: the browser demo, a Node script and a Rust
  integration test hit the same seams — which is exactly what makes conformance
  testable.
- Because the surface is JSON-in/JSON-out, the **engine's canonical serializer** does
  the encoding on both sides: one serializer, one ordering, one set of bytes.
- A host cannot hold a `&mut ReplaySession`; it holds an opaque handle, so it cannot
  reach in and set an outcome.

### Negative
- JSON parse cost per call (small: one attempt per session) and one extra serialization
  hop versus passing structs. Accepted for boundary hygiene.
- DTOs must be kept in sync with the domain by hand; mitigated by round-trip tests and a
  golden DTO snapshot per contract version.

### Neutral
- `session_handle_json` is opaque; its fields are not part of the contract.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| REST + JSON over HTTP (scaffold default) | Familiar | No sockets in wasm; requires a server (non-goal) | Impossible in the target environment |
| `serde-wasm-bindgen` typed structs | Type-safe, no parse | Every DTO change becomes an ABI break; harder to drive from Rust tests | Conformance vectors are easier as JSON |
| `JsValue` free-form objects | Smallest glue | No schema, no versioning, easy to hand in a verdict | Misuse is the threat model |
| One `run()` returning everything | Single call | Verification must accept an artifact obtained elsewhere | The fourth verb is required by FR-006 |

## Implementation

**Affected Modules:** `.pi/architecture/modules/portable-session.md`

**Crate paths:**
- `wasm/src/interfaces/wasm_api.rs` — the four exported functions (ADR-013 removed `verify_artifact_set`)
- `wasm/src/interfaces/dto/*.rs` — `#[serde(deny_unknown_fields)]` DTOs
- `wasm/src/interfaces/errors.rs` — redacted error projection

**Canonical References:** `// @canonical wasm/.pi/architecture/decisions/ADR-004-api-design.md`

## Validation

- **integration-validator**: DTO round-trip per contract version; unknown-field rejection.
- **security-validator**: no backtrace/path/secret in `errors.rs` output.
- **ci-mr-validator**: `contract_version` matches published semver; npm version equals engine version.

## References

- Related ADRs: ADR-001, ADR-002, ADR-005, ADR-010

---

*Decision date: 2026-11-03*
*Decision makers: architecture-coordinator, domain exploration session 3baf6e5c*