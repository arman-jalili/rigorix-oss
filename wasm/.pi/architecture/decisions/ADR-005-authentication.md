# ADR-005: Authentication & Authorization — There Is No Principal; Key Ownership and Capability-Free Surface

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

The template asks for "API Keys + Scopes". That assumes a server with principals. This
crate has **no principal, no session token, no tenant and no network**: it is a library
compiled into the host's page. "Who is calling" is answered by the browser, not by us,
and pretending otherwise would be security theatre.

The real authorization questions are different, and they are sharp:

1. **Can the host make the gate say yes?** It must not — that is invariant 3 (FR-009).
2. **Whose key signs, and what does that signature prove?** In the demo a visitor's key
   is generated in-page; a signature then proves integrity and binding, **not**
   third-party non-repudiation. (Tracked exit condition in the exploration.)
3. **May a host-supplied approval authorise a step?** Only as a claim the engine binds
   to the intent hash; a mismatch is refused (FR-013).

## Decision

**No authentication layer in this crate. Authorization is structural, not credentialled.**

| Concern | Decision |
|---------|----------|
| Principals / tokens / scopes | **Absent by design.** The host is trusted for *what it supplies as data* and for nothing else. |
| Capability surface | Five functions, none of which accepts a verdict, a finding, an `allow` flag or a policy override. There is no "admin" path to disable checks. |
| Key custody | The key source is a **port**. Shipped adapters: (a) host-provided seed key for the demo, (b) `SeededKeySource` for vectors. No key is embedded, logged, or exported. The artifact records only an **opaque key reference** so an auditor knows which key produced the record. |
| Signature meaning | Documented, per key: "proves this artifact set was produced by whoever holds this key and was not altered after signing". Never "the signer is authorised". |
| Approval | A claim, evaluated by the engine against the intent hash. The crate never grants one and models no approver UI (FR-013). |
| Host-influenced refusal | Impossible to suppress: no API takes a "skip" parameter. Policy refusals come from the engine; port failures come from the adapters; both are returned, not filtered. |
| Demo key warning | The demo's limits panel states, in one sentence, that a visitor-generated key gives integrity and binding but not third-party non-repudiation. |

Consequence for the artifact: the stamp records `key_ref` (opaque, e.g. a SHA-256 of the
public key with a `demo:` prefix), never the key, never a path, never a user.

## Consequences

### Positive
- No false assurance: nothing in the crate implies it authenticated anyone.
- The threat that matters — a host forcing an allow — is closed by the *shape* of the
  API rather than by a check that could be forgotten.
- Secrets never exist in the crate's outputs, so NFR-003 is enforceable by scan.

### Negative
- A partner product needing real non-repudiation must bring its own key custody and
  attestation (the engine's identity attestation context); this crate will only carry
  the opaque reference. That is a documented limit, not a defect.
- Absent scopes, the host can call `verify_artifact_set` on any artifact set it holds.
  Accepted: verification is a read-only public operation and needs no authority.

### Neutral
- A future server-side context would introduce principals; it would do so in its own
  ADR, not by retrofitting this contract.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| API keys + scopes in the wasm surface (scaffold default) | Familiar model | A key embedded in a browser bundle is not a secret; scopes over one host are theatre | No server, no principals, no secrecy possible |
| Host-supplied "trusted" flag | Simple | Exactly the authority leak FR-009 forbids | Violates the core invariant |
| Verify only artifacts the crate produced in this session | Prevents odd inputs | An auditor verifies a record obtained elsewhere — that is the whole point | Defeats FR-006 |
| Engine attestation inside the bundle | Strong identity | `ring`/TLS path and key material in the browser; bundle and trust cost | Attestation is the engine's context; borrowed reference only |

## Implementation

**Affected Modules:** `.pi/architecture/modules/portable-session.md`

**Crate paths:**
- `wasm/src/domain/ports/key_source.rs` — `KeySource` trait, `KeyRef` value object
- `wasm/src/infrastructure/adapters/seeded_key_source.rs` — vector adapter
- `wasm/src/infrastructure/adapters/host_key_source.rs` — demo adapter (host-supplied seed)
- `wasm/src/application/export_artifact_set.rs` — stamps `key_ref` only

**Canonical References:** `// @canonical wasm/.pi/architecture/decisions/ADR-005-authentication.md`

## Validation

- **security-validator**: no key material in artifacts, errors or telemetry; `key_ref`
  is opaque; refusal messages contain no principal-shaped claims.
- **architecture-validator**: no API accepts a verdict/override parameter.
- **integration-validator**: approval-claim mismatch yields a refusal (FR-013).

## References

- Related ADRs: ADR-004, ADR-006, ADR-007
- Engine: identity attestation context, `engine/.pi/architecture/decisions/ADR-016-audit-integrity-anchor.md` (envelope signing)

---

*Decision date: 2026-11-03*
*Decision makers: architecture-coordinator, domain exploration session 3baf6e5c*