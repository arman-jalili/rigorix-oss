# ADR-010: Artifact Set Format — Engine-Owned Bytes, Sealed, Stamped, Three-Valued

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

`ArtifactSet` is the only durable output of the crate (ADR-002) and the only input to
verification (FR-005). If the crate invents its own shape, the independent verifier has
nothing to check and conformance with the CLI is moot. If the crate re-serializes the
engine's envelopes, byte identity is lost on the first field-order difference.

Verification also has a known semantic trap (#472/#1285): a boolean cannot distinguish
"compared and does not match" from "could not compare". The second must never accuse.
The same trap exists in the engine's identity vocabulary: a port that could not be
checked is `unverified`, never `unauthenticated`.

## Decision

**The artifact set is a sealed container of engine-produced bytes plus crate-owned
metadata. The crate never re-encodes an envelope.**

Structure (JSON object, canonical serialization by the engine's serializer):

| Field | Owner | Rule |
|-------|-------|------|
| `artifact_format_version` | this crate | Integer; bumped on any change to this container's shape |
| `engine_version` | engine (`ENGINE_VERSION`) | Stamped verbatim; no local constant |
| `scenario_digest` | engine | SHA-256 over the scenario as parsed |
| `fixture_digest` | this crate | SHA-256 over the canonical fixture transcript |
| `session_id` | this crate | Derived deterministically from the digests + seed |
| `key_ref` | this crate | Opaque key reference only (ADR-005) |
| `envelopes` | engine | Signed envelopes, byte-for-byte as the engine emitted them |
| `evidence_refs` | engine | Ordered; refs, not inlined evidence bodies |
| `outcome_digest` | this crate | SHA-256 over the `RunOutcome` projection |

Rules:

1. **Sealed.** Once `ExportArtifactSet` returns, the set is immutable. There is no
   `set_*` method and no re-export that mutates it.
2. **Self-describing.** Everything a verifier needs is inside; nothing is fetched.
3. **Clean.** No secrets, no key material, no absolute host paths, no personal data.
   A pre-export scan rejects the set rather than sanitising it (NFR-003).
4. **Deterministic.** Field order and map ordering are fixed; digests are computed over
   canonical bytes, so two runs of one session are byte-identical (NFR-001).
5. **Three-valued verification.**
   ```rust
   pub enum VerificationResult {
       Verified { key_ref: KeyRef, checked_at_reading: ClockReading },
       NotVerified { reason: MismatchReason, envelope_index: usize },
       Unverifiable { reason: UnverifiableReason },
   }
   ```
   `reason` is populated only for `Unverifiable` (unsupported format version, absent
   key, key not held, malformed input). Unknown `artifact_format_version` ⇒
   `Unverifiable`, never a guess (FR-010, FR-014).
6. **Vocabulary guard.** The interface layer may render only `verified`,
   `not_verified`, `unverifiable`. Words like "tampered", "forged", "unauthenticated"
   are not available to the crate's messages.

## Consequences

### Positive
- The independent verifier sees exactly what the CLI sees; the crate adds an envelope
  around bytes it did not author.
- A record can be carried in a chat message or a download and still verify.
- The three-valued enum makes the accusing/not-checked confusion unrepresentable.

### Negative
- A format change requires a version bump and a migration note for hosts; there is no
  in-place upgrade path for old sets (they verify as `unverifiable` or under their own
  version's rules).
- `unverifiable` forces the host UI to say "we could not check", which is less
  satisfying than a red X. Intended.

### Neutral
- The container is JSON; a binary form would be smaller but the engine's envelopes are
  already JSON text and byte identity is preserved either way.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| Crate defines its own artifact document (re-serialize envelopes) | Full control | Byte identity lost; two serializers | Directly breaks FR-007 |
| Boolean verification | Simple UI | Accuses records that could not be checked | #472/#1285 |
| Include evidence bodies inline | Self-contained evidence | Larger, and duplicates engine data that must stay authoritative | Refs only; bodies live in the engine's store |
| Sign the artifact set itself (double signature) | Crate-level integrity | A signature that proves only "we wrapped it", at real bundle cost | The engine's envelope signature is the one that means something |
| Binary (CBOR/MessagePack) container | Smaller | No human inspection; converter in the verifier path | Debuggability wins at this size |

## Implementation

**Affected Modules:** `.pi/architecture/modules/portable-session.md`

**Crate paths:**
- `wasm/src/domain/artifact/artifact_set.rs` — sealed VOs, `seal()`
- `wasm/src/domain/artifact/verification_result.rs` — three-valued enum
- `wasm/src/application/export_artifact_set.rs` — stamping + pre-export scan
- `wasm/src/application/verify_exported_artifacts.rs` — version gate then bridge
- `wasm/src/infrastructure/verifier_bridge.rs` — the only place the SDK is named

**Canonical References:** `// @canonical wasm/.pi/architecture/decisions/ADR-010-artifact-format.md`

## Validation

- **integration-validator**: artifact set produced in wasm verifies under
  `rigorix-verifier` with no shared code; unknown format ⇒ `unverifiable`.
- **security-validator**: scan of exported bytes (no key, path, PII).
- **ci-mr-validator**: golden artifacts match the CLI byte for byte (ADR-009).
- **test-validator**: `unverifiable_never_renders_as_not_verified`.

## References

- Related ADRs: ADR-002, ADR-004, ADR-005, ADR-007, ADR-009
- Engine `engine/.pi/architecture/decisions/ADR-016-audit-integrity-anchor.md` — the signed audit envelope and hash chain; `docs/spec/terminology.md`

---

*Decision date: 2026-11-03*