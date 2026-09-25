# Architecture Decision Record: [ADR-016]

<!--
Canonical Reference: .pi/architecture/decisions/ADR-016-audit-integrity-anchor.md
Blueprint Source: Guardian Framework v1.2
-->

## Title

Audit integrity model: ledger / projection / cache, deployment modes, and the trust anchor

## Status

- [ ] Proposed
- [x] Accepted
- [ ] Deprecated
- [ ] Superseded by ADR-XXX

**Implementation status (2026-09-25):** Phases **A**, **B** and **C** are
**implemented** — Phase A in OSS (#898, released 1.7.0), Phase B in the enterprise
(#207/#210), Phase C in OSS (#899/#908), with the envelope contract frozen and
independently verifiable (Rust/Python/TypeScript) in rigorix-sdk. **Phase D remains
deferred** (see below).

## Context

The signed audit trail (`.rigorix/audit`, Strategy A) is used for two things: it is
the **forensic record** of governed runs, and it is an **input to policy** — the
cross-run / effect-keyed rules (ADR-013 R7, ADR-014) read prior runs' evidence to
decide whether the current plan may run. That second use makes the trail's
integrity an enforcement property, not just a documentation property.

Today the trail provides **integrity against a party without the key, and nothing
else**. Verified against `main` before this ADR:

| Fact | Evidence |
|---|---|
| The envelope has **no chain/link/sequence** field. Its hashes are unrelated to sibling envelopes. | `engine/src/audit/domain/envelope.rs` — `planning_hash` (plan replay), `intent_hash` (approval binding), `git_commit`; no `prev_hash`/`sequence` |
| The local store is one file per run, named by a random UUID; `delete` and `prune` exist. | `engine/src/audit/infrastructure/local_audit_repository.rs:38` (`{execution_id}.json`), `:157` (`delete`), `:184` (`prune`) |
| The signature is **HMAC-SHA256** (symmetric) over the canonical envelope; the key is held by the agent host. | `engine/src/audit/application/envelope_factory_impl.rs:50`; key from `rigorix.toml audit_hmac_key` / `RIGORIX_HMAC_KEY` (`mcp/src/host/mod.rs:1366`) |
| **Nothing verifies the signature at runtime.** | `verify_signature` (`engine/src/audit/application/factory.rs:38`) has no production caller — tests only |
| The cross-run guard reads envelopes and **trusts them without verifying**. | `engine/src/sequence_policy/infrastructure/history.rs:104` (`prior_actions` → `repo.list`) |
| Enterprise ingestion verifies an HMAC over its **own `record` payload**, not the OSS envelope signature; it stores the envelope verbatim; the store has a `DELETE FROM` path. | `core/src/audit_ingestion/application/dto/mod.rs:76` (called at `interfaces/http/routes.rs:115,159`), `dto/mod.rs:36` (`envelope_jsonb`), `audit_ingestion/infrastructure/repository.rs:288` |
| `.rigorix/**` writes are denied — but **in-band**, at the tool/permission layer, in the agent's own process. | `engine/src/permission/application/enforcer_impl.rs:54-60,166` (R5) |

Consequences:

- **Deletion, reorder, and insertion are undetectable** — there is no chain and no
  sequence, so removing the incriminating prior envelope leaves no trace.
- **A holder of the HMAC key can edit and re-sign, or forge outright.** HMAC is
  symmetric: the host that signs is the host that can fabricate.
- **The guard trusts whatever the local file says** — so "delete the prior run"
  dodges a cross-run rule even though the rule is declared and the trail is
  "signed".
- The **tail-deletion attack is not closable on a single host**: deleting the most
  recent envelope breaks no chain, because nothing follows it.

This is a load-bearing gap: ADR-014's prose calls the retention coupling
"load-bearing", and the cross-run rules are presented as enforcement — but the
evidence they consume is neither verified nor tamper-resistant.

## Decision

### The three tiers

Stop treating "the audit trail" as one artifact. Split it by **trust role**:

| Tier | Home | Properties | Trusted for decisions? |
|---|---|---|---|
| **Ledger** | external anchor (enterprise DB / external sink) | append-only, agent-inaccessible, per-producer sequence + chain, reads signed by the anchor | **yes** |
| **Projection** | a signed read from the ledger | narrow, verifiable ("prior actions in window W for scope S") | **yes** (after verification) |
| **Cache** | local filesystem / `rigorix-server` | fast, rebuildable, chain-linked, tamper-**evident** | **no** |

### Two principles

- **P1 — Evidence authority ≥ rule authority.** Rules sourced from the enterprise
  (the policy bundle, ADR-015/#889) must be evaluated against evidence of at least
  the same authority. Enterprise-authored history rules enforced on local,
  deletable evidence is incoherent.
- **P2 — No symmetric secret across a trust boundary.** HMAC is for *within* one
  trust domain (cache integrity). Across host ↔ anchor, sign asymmetrically or
  evaluate at the anchor.

### The precise claim (guarantee / non-guarantee)

This ADR exists to make the security claim *precise*, so the implementation is not
over-built for a threat it never promised to stop.

**Guaranteed** (in `anchored` mode):

1. A **recorded** event cannot be altered or removed undetectably (append-only +
   per-producer chain + anchor-signed reads).
2. A **registered-but-unevidenced** run is detectable (write-ahead registration).
3. A **fork / rollback / gap** in a producer's chain is detectable.
4. The history the guard consults is **authenticated by the anchor** — the host
   cannot forge it.

**Not guaranteed:**

- The **truthfulness** of a host compromised *before* it registers/records
  (fabricated-at-source evidence). Defending this needs a trusted execution
  boundary (enclave, HSM-held host key, or anchor-side observation) — see Phase D.
- In **`local_unanchored`** mode: anything beyond tamper-evidence of the cache.
  Notably **tail deletion is undetectable** without an external head.

**Adversary boundary.** The threat this model defends is the one the product is
about: an agent that is (or becomes) able to act on the host *after* prior runs
are already recorded, and tries to make the trail forget them. A host that is
compromised *before any evidence exists* is explicitly out of scope.

### Contract (additive; serde-defaulted; code is truth)

Envelope (`engine/src/audit/domain/envelope.rs`, mirrored in `schemas/envelope.json`):

- `producer_id: Option<String>` — the chain this envelope belongs to.
- `sequence: Option<u64>` — monotonic per producer.
- `prev_hash: Option<String>` — hash of the predecessor's canonical bytes (genesis
  marker for the first).
- `history_integrity: Option<HistoryIntegrity>` — `local_unanchored | anchored`,
  recording which regime produced this evidence.

Anchor-signed artifacts (new; owned by the anchor, verified by clients):

- **Ledger receipt** — `{ ledger_id, envelope_hash, sequence, recorded_at, sig }`.
- **History slice** (the projection the guard consumes) — `{ scope, since, actions[], head_hash, sig }`.
- **Compaction checkpoint** — `{ sequence_from, sequence_to, reason, prior_head, new_head, sig }`.

**Signing:** the anchor signs with an **asymmetric** key (Ed25519) so clients verify
with a public key and the agent host cannot forge. HMAC remains the *local* cache
integrity mechanism and is **not** an authority signal.

### Ownership (no policy in the anchor)

| Actor | Owns | Does **not** own |
|---|---|---|
| **Engine / host** | all policy evaluation (ADR-013/014/015), the cache, the anchor client | authority over history |
| **Anchor** (enterprise Execution API) | registry (write-ahead), ledger (verify/sequence/chain/append-only), signed reads, compaction | ❌ any sequence-policy / matcher / verdict |

The engine's `ExecutionHistory` port is the seam: `EnvelopeHistoryAdapter` (local,
unanchored) or `AnchoredHistoryAdapter` (verifies an anchor-signed slice). **The
matcher never changes.** The enterprise `PolicyBundleService` remains
distribution-only (rules authored by the enterprise, enforced by the engine).

### Modes and behavior

- `local_unanchored` — no anchor configured (OSS default). The cache is
  tamper-evident; envelopes are tagged `history_integrity = local_unanchored`.
  **Deny-class cross-run rules refuse by default** when history is unanchored; a
  loud, recorded opt-in (`history_policy = allow_unanchored`) restores best-effort
  behavior for local dev/demos.
- `anchored` — an anchor is configured. History-dependent rules read the
  anchor-signed projection; **fail closed** for consequential runs when the anchor
  is unreachable. `rigorix.system.version` reports the active mode.

### GDPR / deletion

"Never delete" is not shippable (erasure rights, cost). Deletion becomes a
**recorded compaction**:

- Erase the **payload** via key destruction (crypto-shred); keep
  `{ producer_id, sequence, prev_hash, erased: true }`.
- Cover the erased range with a **signed compaction checkpoint**; the chain links
  *across* the gap.
- A verifier accepts a gap **iff** it is covered by a valid signed checkpoint; an
  uncovered gap is a failure.

This also gives ADR-014's retention coupling (GAP-A-29) a real foundation: a
retention shorter than the longest effect-keyed window is refused (the evidence
will be erased), and the erasure is provable rather than silent.

### Non-goals (explicit)

- Host-side HSM/enclave/key custody and anchor-side observation (**Phase D**;
  deferred — no requirement yet).
- A Merkle tree / transparency-log / gossip stack. A **linear per-producer chain +
  signed checkpoints** is sufficient.
- Any policy, matcher, or verdict inside the anchor/enterprise.
- Global ordering across producers, or consensus.
- Real-time streaming; an anchor dependency for non-consequential runs.
- Defending a host compromised before any evidence is recorded (the out-of-scope
  boundary above).

## Alternatives Considered

| Alternative | Why rejected |
|---|---|
| Keep HMAC-only, add nothing (status quo) | Symmetric across a trust boundary: the key holder forges. Deletion is undetectable. Not an enforcement control. |
| Put the trail in the DB, keep HMAC verification | The DB would faithfully store forgeries, and a withholding host leaves no gap. Authority needs attestation + sequence + write-ahead, not just a database. |
| Evaluate sequence policy **inside** the enterprise | Duplicates the ADR-013 matcher in a second codebase (exactly what ADR-013 centralized), makes the anchor a hard dependency for every run, and there is no requirement for it. |
| Merkle/transparency-log now | Stronger than needed; the threat is closed by a chain + anchor-signed reads. Cost without a requirement. |
| OS-isolated local store only (separate UID, append-only mount) | Deployment-specific, platform-fragile, and still same-host. Documented as a deployment option, not a product guarantee. |

## Consequences

### Positive

- Deletion, reorder, and insertion become detectable; the cross-run guard consumes
  **authenticated** evidence.
- GDPR-compatible erasure that does not break the chain.
- The enterprise is a **product tier** (the anchor), not a bolt-on: the guarantee
  lives where an external trust domain exists.
- No policy duplication; one matcher; the engine stays the single evaluator.
- OSS stays honest and shippable: local-only is tamper-evident and labelled as
  such, not sold as tamper-proof.
- GAP-A-29 (retention) gains a coherent foundation.

### Negative

- The `anchored` guarantee depends on an external anchor (latency, availability) —
  mitigated by mode-scoped fail-closed behavior.
- New contract surface (envelope fields + three anchor-signed artifacts) that must
  be frozen in rigorix-sdk before both servers implement it.
- Write-ahead registration adds a call before consequential runs.
- Signed compaction adds verifier complexity (gaps are proofs, not absences).

## Implementation

**Phase A — local honesty + contract (OSS + SDK, no anchor).** ✅ **Implemented** (#898).
Freeze the contract (this ADR + SDK schemas); add `verify-on-read` in the guard and
`rigorix_read_audit`; add the local chain (`sequence`/`prev_hash`) and verify it;
tag envelopes `local_unanchored`; make deny-class cross-run rules refuse by default
when unanchored. Wire the ADR-014 retention validator (GAP-A-29) fail-closed.

**Phase B — the ledger (enterprise Execution API), no policy.** ✅ **Implemented** (#207/#210).
Verify the envelope at ingestion; assign/verify `sequence`; store `prev_hash`;
append-only; write-ahead registration (the existing `RunRegistration` lifecycle, plus
a "registered, evidence overdue" state); anchor-signed history reads; payload
erasure + signed compaction checkpoints.

**Phase C — anchored mode (OSS `rigorix-server` + engine).** ✅ **Implemented** (#899/#908).
`AnchoredHistoryAdapter` behind `ExecutionHistory`, verifying the anchor's
signature; `history_integrity = anchored`; fail closed for consequential runs when
the anchor is unreachable; `rigorix.system.version` reports the mode.

**Phase D — hardening (DEFERRED; trigger = a requirement to defend a host
compromised before evidence exists).**
Trusted execution boundary: host-side asymmetric signing with the key in an
HSM/enclave, or anchor-side observation of the run. Not built now: no requirement,
and it does not affect the in-scope threat.

## Validation

| # | Criterion | Phase |
|---|-----------|-------|
| 1 | A tampered cache envelope is **rejected** by the guard and by `rigorix_read_audit` (verify-on-read) | A |
| 2 | A deleted/reordered/inserted envelope is detected by the local chain (interior), and by the anchor (any), including the **tail** | A/C |
| 3 | A deny-class cross-run rule **refuses** when unanchored (default), and the opt-in is recorded in the envelope | A |
| 4 | An anchor-signed history slice is verified before matching; a forged slice is rejected | C |
| 5 | A registered run with no evidence is **detectable** at the anchor | B |
| 6 | A fork/rollback in a producer's chain is rejected at ingestion | B |
| 7 | Erasure is covered by a signed checkpoint; an uncovered gap fails verification | B |
| 8 | `rigorix.system.version` reports `history_integrity`, and the anchored mode fails closed when the anchor is down | C |
| 9 | The enterprise exposes **no** policy/matcher/verdict surface (catalog scan) | B |
| 10 | No host-side asymmetric key or enclave is required for Phases A–C | A–C |

## References

- **ADRs:** ADR-011 (approval binding), ADR-012 (identity attestation), ADR-013
  (sequence policy), ADR-014 (effect-identity + retention coupling), ADR-015
  (operator step requirements)
- **Code (current state):** `engine/src/audit/domain/envelope.rs`,
  `engine/src/audit/application/envelope_factory_impl.rs` (`compute_signature`),
  `engine/src/audit/infrastructure/local_audit_repository.rs`,
  `engine/src/sequence_policy/infrastructure/history.rs`,
  `engine/src/permission/application/enforcer_impl.rs` (R5),
  `mcp/src/host/mod.rs` (key resolution)
- **Contract home:** `rigorix-sdk schemas/envelope.json` + `rust/rigorix-verifier`
- **Enterprise:** `core/src/audit_ingestion/**` (ingestion, `envelope_jsonb`,
  repository), `core/.pi/architecture/modules/execution-api.md` (`RunRegistration`
  lifecycle)
- **Gaps:** GAP-A-29 (retention coupling — validator unwired), GAP-A-30 (audit
  integrity — no chain/anchor; this ADR)
- **Backlog / strategy:** `docs/sdk-server-strategy-freeze-2026-09-06.md` (D-2
  Strategy A, D-3 server mode), `docs/next-workstreams-2026-09-21.md` (C series)
