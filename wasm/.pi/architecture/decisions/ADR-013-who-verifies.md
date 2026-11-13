# ADR-013: Who Verifies — the Crate Exports the Record, the Host Verifies It

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

The exploration gave the crate an application service, `VerifyExportedArtifacts`, and
criterion 13 required that a wasm-produced artifact set verify *"under `rigorix-verifier`
with no shared code path"*. Both were left ambiguous about **where the verification code
runs**, and the two readings are opposite designs:

- **In-module**: the Rust verifier is compiled into the same wasm bundle as the gate, so
  the bridge "parses, delegates, maps" inside the same module. Produces a tidy JS API.
- **Host-side**: the crate only produces and exports the artifact set; the host calls the
  independent SDK in JavaScript.

Measured facts that decide it:

- npm `rigorix-verifier` 0.9.0 is a **TypeScript reimplementation** (`canonical.ts`,
  `envelope.ts`, `chain.ts`, `anchor.ts`, `corpus.ts`) — not a wrapper over Rust. The
  Rust verifier lives separately under `rigorix-sdk/rust/rigorix-verifier`.
- The whole demo narrative rests on *"the recipient verified the record with an SDK that
  shares no code with the platform"*. A verifier compiled into the producer's own module
  makes that sentence false by construction, in the one artifact the demo exists to show.

## Decision

**The crate does not verify anything. It exports the record and the stamp; verification is
the host's act, performed with the independent SDK.**

| Concern | Owner |
|---------|-------|
| Produce envelopes, bind evidence, seal the run | criterion-level crate behaviour (engine logic) |
| Serialize `ArtifactSet` + `artifact_format_version` + `engine_version` + digests | crate (`ExportArtifactSet`) |
| Decide whether the record is authentic | **host**, via `rigorix-verifier` in JavaScript |
| Render the three-valued result (`verified` / `not_verified` / `unverifiable(reason)`) | host |

Consequences for the published surface, which shrink from five functions to four:

```
open_session(scenarioRef, fixtureWorld) -> handle
attempt_run(handle, action)             -> RunOutcome
export_artifact_set(handle)             -> ArtifactSet
contract_version()                      -> { format, engine }
```

`VerifyExportedArtifacts` is removed as an application service and restated in the host
contract as an obligation on the host, with the three-valued vocabulary it must render.
`VerifierBridge` is removed from `infrastructure/`; there is nothing to bridge.

The independence claim becomes **provable rather than asserted**: an architecture test
asserts the wasm bundle's dependency tree contains no verification code (no `rigorix-verifier`,
no `sha2`/`hmac`/`ed25519-dalek` used for verification — signing is the production side and
remains), and the demo's verification runs in a JS realm that shares only the file format.

## Consequences

### Positive
- The demo's central sentence is true by construction: the verifier is not in the producer.
- Smaller bundle (NFR-002): the verifier's dependency subtree never enters the wasm build.
- One less module, one less port, one less thing to keep in lockstep with the SDK.
- The crate stays a *producer*; the SDK stays an *independent checker* — the separation the
  product sells.

### Negative
- The JS surface is one call larger for the host (it must import two packages). Acceptable:
  the demo already vendors both.
- A host that forgets to verify gets no verdict at all — so the host contract must state
  that "the crate produced it" is not "it verifies", and the demo must always show the
  verification step as a separate act with its own visual moment.

### Neutral
- Signing keys remain a port; verification needs the matching public key, supplied by the
  host at verification time, never by the crate.

## Alternatives Considered

- **In-module Rust verifier.** Rejected: it puts the checker inside the producer, which is
  precisely the claim the product makes about everybody else. It also drags the verifier's
  dependencies into a size-budgeted bundle.
- **Both paths, feature-gated.** Rejected: two verification implementations that can disagree
  is a second source of truth for the most consequential word in the product.
- **Crate returns only raw bytes and no stamp.** Rejected: the stamp (format version,
  engine version, scenario and fixture digests) is what lets a host decide *which* verifier
  to use and refuse an unknown format instead of guessing.

## Implementation

1. Drop `VerifyExportedArtifacts` from the application layer and `VerifierBridge` from
   infrastructure; amend FR-006 and criterion 13 accordingly.
2. Host contract documents: four functions, the stamp fields, and the host's verification
   obligation with the three-valued vocabulary.
3. Architecture test: the wasm bundle's dependency tree contains no verifier crate.
4. Demo wires `rigorix-gate` (produce) and `rigorix-verifier` (check) as separate imports,
   with the verification step presented as its own act.

## Validation

- `cargo tree --target wasm32-unknown-unknown -p rigorix-gate-wasm` names no verifier.
- A wasm-produced artifact set verifies in Node with `rigorix-verifier` alone, in a process
  that has never loaded the wasm module.
- An edited byte in the exported set flips the host verdict to `not_verified`; a set with an
  unknown format version yields `unverifiable(UnsupportedFormatVersion)`.

## References

- Exploration FR-006 (amended), FR-014; use case `VerifyExportedArtifacts` (removed)
- `rigorix-sdk/typescript/src/{canonical,envelope,chain}.ts` — the verifier the host uses
- ADR-010 (artifact format), ADR-005 (key ownership and capability-free surface), ADR-002 (no persistence)
