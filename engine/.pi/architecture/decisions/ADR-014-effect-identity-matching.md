# Architecture Decision Record: [ADR-014]

<!--
Canonical Reference: .pi/architecture/decisions/ADR-014-effect-identity-matching.md
Blueprint Source: Guardian Framework v1.2
-->

## Title

Effect-identity matching for sequence policy: bounded relation predicates and effect-keyed history

## Status

- [ ] Proposed
- [x] Accepted
- [ ] Deprecated
- [ ] Superseded by ADR-XXX

## Context

Sequence policy (ADR-013) matches ordered steps within a run and, for cross-run
patterns, looks back over the signed history of completed runs. Today the matcher is
deliberately narrow:

- `StepPredicate` matches a tool name (exact or glob) plus optional `ParamPredicate`
  entries. Each `ParamPredicate` compares the value at a JSON pointer against a
  **declared literal**, using `Exact`, `Glob` or `Regex` (`ParamMatchKind`).
- `SequenceRule.steps` is matched over the ordered step list with adjacency and an
  optional window, so a pair may be non-consecutive but never reordered.
- `HistoryPredicate` carries exactly three fields: `prior_node`, `same_principal`,
  `window_secs`. Cross-run matching therefore keys on **prior step type + acting
  principal + time window**.
- Signed audit envelopes deliberately carry **no step parameter values** in their
  summaries (SpanPrivacy default); full payloads are opt-in and never leave the
  local store.

This leaves a real class of pattern inexpressible. The strongest form of
"each step was allowed, the sequence was not" is requester-side variation toward an
identical **effect**: three payouts to three different account identifiers that
resolve to the same beneficiary; a removal and a re-registration that name the same
resource differently; a data export split across three calls that target one
subject. Each step passes the gate; the effect is the same each time.

Two distinct gaps produce that blindness:

1. **No relational predicate.** Predicates compare a value against a declared
   literal, never against another step's value. A rule cannot say "this step's
   `/beneficiary` equals the `/beneficiary` of an earlier matched step".
2. **No effect-keyed history.** Cross-run matching keys on the acting principal,
   not on the target of the effect. "A prior action with the same effect, by this
   principal, inside the window" is not expressible.

## Decision

Bound the capability to two predicates inside this layer, and keep identity
resolution and analytics outside it.

**Layer 1 — outside rigorix (domain / adapter).** Entity resolution is not this
layer's job. The domain integration resolves the effect identity against a system
of record the agent cannot influence (core system, PSP, registry) and supplies a
**canonical effect key** with the step. Similarity, aliasing and fuzzy matching stay
in the domain.

**Layer 2 — this layer (bounded, deterministic).**

1. **Value-identity predicate (within a run).** A new `ParamMatchKind` variant
   meaning "equal to the value at pointer P in an earlier matched step of this run"
   (e.g. `/beneficiary` equals the earlier step's `/beneficiary`). Structural only:
   it compares values the run already carries, requires no domain knowledge, and is
   evaluated over the same ordered-step window semantics as existing rules.
2. **Effect-keyed history (across runs).**
   - `AuditEnvelope` gains an additive, serde-defaulted **effect key** field: a
     domain-supplied opaque value, stored as a one-way hash keyed by the domain
     secret, so effect comparison across runs is possible without recording raw
     parameters.
   - `HistoryPredicate` gains an optional effect-key match: "a prior completed run
     recorded this same effect key, for this principal, inside `window_secs`".
   - **Reserved carrier:** the current run's effect key is read from the matched
     step's parameters at the reserved pointer **`/effect_key`** (a string).
     Entity resolution happens outside rigorix (Layer 1); this layer only reads
     the opaque key. An envelope/step without it never matches an effect-keyed
     rule. The field is populated at the envelope boundary
     (`BuildEnvelopeInput.effect_key`) by the composition root / domain adapter.

**Layer 3 — outside rigorix (analytics).** Aggregation, counters over many
dimensions, scoring and anomaly detection run over the exported trail, offline, and
feed reviewed findings back as policy. They do not run in the enforcement path.

### Non-goals (explicit)

- Entity resolution or identity joining of any kind inside this layer.
- Fuzzy / similarity matching on effect keys.
- Aggregations, group-by, per-target counters, thresholds over many dimensions.
- Any scoring or model inference in the enforcement decision (ADR-013, ADR-011,
  ADR-007).

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| Keep matching on declared literals only (status quo) | No schema change, no new predicates, zero risk | The effect class stays inexpressible; rules must enumerate requester-side variations, which the requester controls | Fails the pattern class this ADR exists to address |
| Let the policy layer resolve identity itself | One place to configure | Requires domain data this layer does not own; entity resolution is fuzzy and unbounded; makes the enforcement path depend on heuristic joins | Violates the layer boundary; a fuzzy join cannot be defended as a control |
| Push all effect matching to offline analytics | Keeps the enforcement path minimal | Detection only — the action has already executed; no gate, no pause, no evidence of prevention | Enforcement value is lost; analytics remains complementary, not substitutive |
| Relational predicate + effect-keyed history (this ADR) | Expresses the class; deterministic; no domain semantics in the layer; auditable | Envelope schema addition; retention must cover the longest rule window; the capability is only as good as the domain-supplied key | Chosen |

## Consequences

### Positive

- The requester-variation class becomes expressible without enumerating variants.
- The layer stays free of domain semantics: it compares opaque keys, it does not
  know what a beneficiary is.
- Effect keys recorded as keyed hashes keep the privacy posture of the existing
  envelope (no raw parameters on the wire).
- Deterministic and testable: predicate evaluation is a pure function of the
  ordered steps and the retained trail.

### Negative

- Envelope schema change (additive, serde-defaulted, backwards compatible).
- Retention coupling becomes load-bearing: pruning below the longest rule window
  silently disables effect-keyed rules and must be validated.
- Effect-key matching is only as good as the domain-supplied key. Where no canonical
  key exists, the rule degrades to a heuristic over available attributes and MUST be
  described as detection, not as a control.

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/audit.md` (envelope schema: effect key field)
- sequence-policy module (matcher, rule schema, history adapter)

**Files to Update:**
- `engine/src/sequence_policy/domain/rule.rs` (`ParamMatchKind`, `HistoryPredicate`)
- `engine/src/sequence_policy/application/matcher.rs` (relational predicate,
  effect-keyed history matching)
- `engine/src/audit/domain/envelope.rs` (additive effect-key field)
- `engine/src/audit/application/envelope_factory_impl.rs` (populate effect key)
- `engine/src/sequence_policy/infrastructure/repository/toml_repository.rs`
  (rule schema parsing + fail-closed on malformed effect predicates)

**Canonical References:**
Implementation files should reference: `.pi/architecture/decisions/ADR-014-effect-identity-matching.md`

## Validation

**Validators Required:**
- architecture-validator: confirm no domain semantics enter the matcher; layer
  boundaries preserved
- security-validator: effect key must be a keyed hash, never raw parameters; no new
  data leaves the local store by default

**Acceptance Criteria**

- [ ] Within-run value-identity predicate matches a pair whose pointer values are
      equal and does not match when they differ.
- [ ] Value-identity predicate obeys existing adjacency / window semantics and never
      matches across a reordering.
- [ ] Effect-keyed history rule fires on equal effect keys inside the window, for the
      same principal, and does not fire outside the window or for a different key.
- [ ] An envelope produced without an effect key remains valid and never matches an
      effect-keyed rule.
- [ ] Missing or unreadable history continues to fail closed.
- [ ] Retention shorter than the longest rule window is **enforced at composition**
      (GAP-A-29): when `RIGORIX_AUDIT_RETENTION_SECS` is configured, the real config-load
      path (`SequencePolicySetup::from_env` → `RetentionCoupledSequencePolicyRepository` →
      `SequencePolicyConfig::validate_retention`) refuses an `effect_key` rule whose
      `window_secs` outlives retention (fail closed); with retention unset it is unlimited
      (`Ok`, status quo). A validator with no caller does **not** satisfy this criterion
      (no silent disablement).
- [ ] No aggregation, counting or scoring appears in the matcher (enforced by test
      naming + review).

## References

- Related ADRs: ADR-013 (sequence policy — no LLM judgment in the enforcement
  path), ADR-011 (approval binding — rejected JANUS-style decision machines),
  ADR-012 (identity attestation), ADR-007 (risk gating — LLM-determined risk
  rejected as non-deterministic)
- Related documents: `.pi/architecture/modules/audit.md`

---

*Decision date: 2026-09-15*
*Decision makers: Arman Wolkensteiner-Jalili (author/owner), Hermes (analysis)*
