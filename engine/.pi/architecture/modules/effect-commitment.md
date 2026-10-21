# Effect Commitment Architecture

<!--
Canonical Reference: .pi/architecture/modules/effect-commitment.md
Blueprint Source: Guardian Framework v1.2
Generated: NEVER (this is the source)
-->

## Overview

Owns the **unsettled interval** between dispatching an effect and learning whether it committed:
the write-ahead intent record, the four-state machine with a non-terminal `Ambiguous`, the
resolver authority, and the fences that stop an unsettled effect from being retried or closed.

It explicitly does **not** own dispatch (`execution-engine`), reservations (`capacity-ledger`),
or evidence storage (`audit`). It answers one question — *was this effect committed?* — and its
authority for that answer is **external**: the provider's receipt or read-back. See ADR-019.

**Guarantee ceiling:** the record and the fences are provable in-process (L1). Cross-process
settlement needs a contested store (L2). Any claim that an effect *committed* needs provider
evidence (L3); without a receipt reader this context can only ever prove **not-settled**.

## DDD Layers

| Layer | Purpose | Tech |
|-------|---------|------|
| `domain/` | Intent, state machine, observed outcome, resolver identity | Zero framework imports |
| `application/` | Use cases, key derivation, reconciliation policy | Rust, async |
| `infrastructure/` | In-memory + HTTP adapters, provider receipt reader | std sync, reqwest |
| `interfaces/` | The `EffectCommitmentLedger` port | Rust traits |

**Dependency rule:** `domain ← application ← infrastructure` (inward); `interfaces/` is the
contract consumed by `execution-engine`.

## Components by Layer

#### Domain Layer (`domain/`)
| Component | Description | Framework? |
|-----------|-------------|------------|
| EffectIntent | Identity of a proposed effect: tool, args digest, gate digests | ❌ No |
| EffectState | `Dispatched | Committed | NotCommitted | Ambiguous` | ❌ No |
| EffectRecord | The aggregate: intent + state + outcome + resolver | ❌ No |
| ObservedOutcome | Provider evidence: receipt id, read-back digest, observed_at | ❌ No |
| ResolverIdentity | Who may settle, and under which evidence digest | ❌ No |
| CommitmentError | Closed error vocabulary | ❌ No |

#### Application Layer (`application/`)
| Component | Description | Framework? |
|-----------|-------------|------------|
| EffectCommitmentService | begin / settle_definite / mark_ambiguous / resolve / query | Rust async |
| IntentKeyDerivation | Intent-derived idempotency key | Rust |
| ReconciliationPolicy | Which transitions are legal, and who may make them | Rust |

#### Infrastructure Layer (`infrastructure/`)
| Component | Description | Framework? |
|-----------|-------------|------------|
| InMemoryEffectLedger | Single-process adapter (L1) | std sync |
| HttpEffectLedger | Fail-closed transport to a contested store (L2) | reqwest |
| ProviderReceiptReader | Anti-corruption adapter over a foreign payment system (L3) | reqwest |
| EffectCommitmentConfig | `.rigorix/effect-commitment.toml` + env | Rust |
| DefaultEffectCommitmentFactory | Construction from config | Rust |

#### Interfaces Layer (`interfaces/`)
| Component | Description | Framework? |
|-----------|-------------|------------|
| EffectCommitmentLedger | The port consumed by the dispatch seam | Rust trait |

#### Wiring (the components that make the context reachable)
| Component | Description | Framework? |
|-----------|-------------|------------|
| DispatchSeam | Write-ahead + settle around `spawn_concurrent_node` | Rust async |
| DependentFence | Refuses to release dependents of an `Ambiguous` effect | Rust async |
| RetryFence | Refuses a retry while `Ambiguous` | Rust async |
| NodeStateVocabulary | The cross-context edit: `Dispatched` / `Ambiguous` in `NodeStatus` | Rust |
| CompositionRoots | cli / mcp / actions wiring via `EffectCommitmentSetup::from_env` | Rust |

## Component Details

### EffectIntent

status: planned
depends: none

**Purpose:** The write-ahead identity of a proposed effect. Carries `dag_id`, `node_id`,
`attempt`, the **intent-derived** `idempotency_key`, the tool and args digest, and the
`precondition_digest` / `authority_digest` already produced by the pre-dispatch gate. Written and
made durable **before** the effect is sent; if that write fails, the effect must not be sent.

**Layer:** `domain/`

### EffectState

status: planned
depends: none

**Purpose:** `Dispatched`, `Committed`, `NotCommitted`, `Ambiguous`. `is_terminal()` is false for
`Dispatched` and `Ambiguous`; `blocks_dependents()` and `blocks_retry()` are true for `Ambiguous`.
Transitions are enforced in the type, not by convention — mirroring `ReservationState`, whose
`Ambiguous` semantics this deliberately reuses.

**Layer:** `domain/`

### EffectRecord

status: planned
depends: EffectIntent, EffectState

**Purpose:** The aggregate. Holds the intent, the current state, the observed outcome when one
exists, and the resolver identity that settled it. A terminal state without either an observed
outcome or an explicit `NotCommitted` observation is unrepresentable.

**Layer:** `domain/`

### ObservedOutcome

status: planned
depends: none

**Purpose:** Evidence from the provider: receipt id, read-back digest, observed timestamp, source
identity. This is the only thing that may move `Ambiguous` to a terminal state. A local guess is
not an `ObservedOutcome`.

**Layer:** `domain/`

### ResolverIdentity

status: planned
depends: none

**Purpose:** The authenticated identity permitted to settle an `Ambiguous` effect, plus the evidence
digest it presented. Recorded on the effect so the signed record shows *who* resolved *on what
evidence*. The dispatching process and the agent are rejected by policy, not by convention.

**Layer:** `domain/`

### CommitmentError

status: planned
depends: none

**Purpose:** Closed vocabulary: `AlreadySettled`, `NotAmbiguous`, `UnauthorizedResolver`,
`EvidenceMissing`, `Unavailable`, `Contended`. No catch-all variant.

**Layer:** `domain/`

### EffectCommitmentService

status: planned
depends: EffectRecord, EffectCommitmentLedger, IntentKeyDerivation, ReconciliationPolicy

**Purpose:** `begin`, `settle_definite`, `mark_ambiguous`, `resolve`, `query`. Fail-closed: an
unreachable ledger refuses the dispatch rather than proceeding unrecorded, because proceeding is
exactly the window this context exists to close.

**Layer:** `application/`

### IntentKeyDerivation

status: planned
depends: EffectIntent

**Purpose:** Derives `IdempotencyKey = H(dag_id, node_id, intent_digest)`. Replaces the
attempt-derived key at `dispatch.rs:795`, which makes every retry a new intent and therefore makes
the retry fence unenforceable. Same key + same payload ⇒ read the existing record (the custody
property); same key + different payload ⇒ refuse (preserves #1052).

**Layer:** `application/`

### ReconciliationPolicy

status: planned
depends: EffectState, ObservedOutcome, ResolverIdentity

**Purpose:** Encodes F3/F4: only a resolver presenting an `ObservedOutcome` may leave `Ambiguous`;
the dispatching process and the agent are never that resolver. Pure function, no I/O, so the rule
is testable without a database.

**Layer:** `application/`

### InMemoryEffectLedger

status: planned
depends: EffectCommitmentLedger

**Purpose:** Single-process adapter for L1: exact within one process, meaningless across processes.
Same honesty rule as the capacity ledger's in-process adapter — no deployment may claim L2 while
running this.

**Layer:** `infrastructure/`

### HttpEffectLedger

status: planned
depends: EffectCommitmentLedger

**Purpose:** Transport to a contested store that owns atomicity (L2). Every failure — unreachable,
timeout, contended — is fail-closed: the dispatch is refused, never silently allowed.

**Layer:** `infrastructure/`

### ProviderReceiptReader

status: planned
depends: ObservedOutcome

**Purpose:** The anti-corruption adapter over the foreign payment system: read-back a receipt and
produce an `ObservedOutcome`. This is the L3 boundary. Without it the context can prove
*not-settled* only, and the runbook must say so.

**Layer:** `infrastructure/`

### EffectCommitmentConfig

status: planned
depends: none

**Purpose:** `.rigorix/effect-commitment.toml` plus env. **Absent declaration ⇒ no adapter, no
network, no shared state, unchanged previous code path** — the same opt-in rule as capacity. A
malformed declaration is a hard error at composition time.

**Layer:** `infrastructure/` + `application/setup.rs`

### DefaultEffectCommitmentFactory

status: planned
depends: EffectCommitmentLedger

**Purpose:** Concrete construction from a backend selection. The capacity ledger's contract freeze
declared a factory with **no implementation** and the whole context was unreachable; this exists so
that failure cannot repeat.

**Layer:** `infrastructure/factory_impl.rs`

### EffectCommitmentLedger

status: planned
depends: none

**Purpose:** The port: `begin`, `settle_definite`, `mark_ambiguous`, `resolve`, `query`. Engine
owns the contract; storage owns the atomicity.

**Layer:** `interfaces/`

### DispatchSeam

status: planned
depends: EffectCommitmentService, IntentKeyDerivation, EffectIntent

**Purpose:** The seam. Inside `run_dispatch_loop`, after the precondition/sequence/capacity gates
and immediately before `spawn_concurrent_node`: derive the intent key, `begin` (durably). After the
node returns: `settle_definite` on an observed outcome, `mark_ambiguous` on a lost response,
timeout, or cancellation while in flight. A failed `begin` refuses the dispatch. This is the
component whose absence would leave the context compiled and unreachable.

**Layer:** `application/` + `execution_engine/application/service_impl/dispatch.rs`

**Test:** `tests/effect_commitment_composition_e2e.rs` — kills the process between `begin` and the
response; asserts the record exists, the state is `Ambiguous`, retry is refused, dependents are not
released, and the losing process cannot resolve it. Fails if the seam call is removed (verified by
removal).

### DependentFence

status: planned
depends: EffectState

**Purpose:** `release_or_skip_dependents` must not release a dependent while any upstream effect is
`Ambiguous`. Otherwise the run proceeds on top of an unknown — the exact double-spend path.

**Layer:** `application/` + `dispatch.rs`

### RetryFence

status: planned
depends: EffectState

**Purpose:** The retry policy refuses to re-dispatch an `Ambiguous` effect (F2). A new attempt after
resolution is permitted; an attempt before resolution is not.

**Layer:** `application/` + `dispatch.rs`

### NodeStateVocabulary

status: planned
depends: EffectState

**Purpose:** The one edit this context requires in another: `NodeStatus` gains `Dispatched` and
`Ambiguous` (`state_persistence/domain/state.rs:91`), so a node whose outcome is unknown no longer
has to be recorded as `Completed` or `Failed`. `is_terminal()` excludes both. Without this the
context has nowhere to put its answer.

**Layer:** `state_persistence/domain/state.rs`

### CompositionRoots

status: planned
depends: DispatchSeam, EffectCommitmentConfig, DefaultEffectCommitmentFactory

**Purpose:** `EffectCommitmentSetup::from_env` wired in `cli`, `mcp` and `actions`, exactly where
`CapacitySetup::from_env` is wired today. The capacity ledger was once dead code because no
composition root constructed an adapter; this component exists to make that class of failure visible
in the architecture rather than discovered in a demo.

**Layer:** `cli/src/cli_boundary/orchestrator.rs`, `mcp/src/host/mod.rs`, `actions/src/main.rs`

## Acceptance Criteria

1. A record is durable before the effect is sent; if the write fails, the effect is not sent.
2. An effect with no observed outcome is `Ambiguous`, never `Completed` or `Failed`.
3. `Ambiguous` blocks dependents (F1) and retry (F2), each with a test that fails if the fence is
   removed.
4. Only a resolver presenting an `ObservedOutcome` may leave `Ambiguous` (F3).
5. The dispatching process and the agent are rejected as resolvers (F4), with a test.
6. The idempotency key is intent-derived; a same-key retry reads the existing record and does not
   re-send.
7. No declaration ⇒ no adapter, no network, no shared state; the previous code path is unchanged.
8. The claim ceiling is stated in the runbook: L3 requires provider evidence.

## Dependencies

### Depends On
- `state-persistence` — durable record storage
- `execution-engine` — the dispatch seam
- `audit` — the resolved record becomes evidence
- `capacity-ledger` — shares the `Ambiguous` vocabulary; does not share the record

### Used By
- `execution-engine` — fences dependents and retries
- `orchestrator` — closure of a run that contains an unsettled effect
- `audit` — the observed outcome is part of the signed chain

## Related ADRs

- ADR-019 — this decision
- ADR-017 — consequence gating (check before dispatch; this is the complement, after dispatch)
- ADR-018 — capacity ledger (`Ambiguous` for reservations; this is `Ambiguous` for effects)
- ADR-016 — audit ledger
- ADR-014 — effect identity (intent-digest derivation)
- ADR-013 — write-deny boundary (resolver evidence must live outside the agent-writable workspace)

## Implementation Status

Designed. No code. The epic is intended to be generated by `/architect --epic` from this module
plus ADR-019; no issues are filed by hand, because the component list above is the work breakdown.

## Operability

An `Ambiguous` effect blocks a run indefinitely. That is the guarantee, not a defect, and it makes
two operator surfaces mandatory rather than optional:

- **query ambiguous** — what is unsettled, for which run, since when, with which intent digests
- **resolve** — settle with provider evidence, recording the resolver identity

A timeout that guesses is not a substitute: guessing is how the window is closed incorrectly.

## Guarantee ceiling

| level | guarantee | requires |
|---|---|---|
| L1 | record written before dispatch; fences hold | in-memory adapter |
| L2 | survives process death; settles through a contested store | HTTP adapter |
| L3 | **the effect committed** | provider receipt / read-back |

Without a `ProviderReceiptReader`, the context can prove **not-settled** and never **settled**.
That asymmetry is the honest claim and must appear wherever the feature is described.
