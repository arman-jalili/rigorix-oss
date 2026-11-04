# Gate Architecture

> **Module:** `engine/src/gate/`
> **Bounded context:** consequence gating — *may this step happen, must it wait, or must it not*
> **ADRs:** [ADR-013](../../../docs/architecture/decisions/) (the crate produces, the host verifies),
> ADR-014 §2 (pinned inputs), ADR-015 (operator step requirements), ADR-017 (consequence gating)
> **Issue:** #1381 (Phase C.1), #1406 (this definition)

## Overview

One gate decision, computed once, called by two runtimes. The `precondition` context decides whether
a step may dispatch at all; the `sequence_policy` context decides which operator obligations the step
leaves unmet. Neither owns the **order** they run in, and neither owns the **language of the answer** —
and the first version of this module claimed it owned neither, which is what put the answer's language
in `application/dto.rs` and produced a module with one layer (#1406).

This module owns two things:

1. **The order.** Preconditions first, requirements second, fail closed at the first refusal. Two
   callers that each order the two services themselves agree only by imitation; one function means
   there is nowhere for the order to drift.
2. **The decision's language.** Three verdicts (`Dispatch`, `Promote`, `Refuse`); an outcome that
   distinguishes *refused* from *could not evaluate*; and a digest over `{verdict, findings}` and
   nothing ambient. Those are invariants no other context states.

## DDD Layers

| Layer | Purpose | Tech |
|-------|---------|------|
| `domain/` | The decision's language and invariants: verdict, outcome, finding, decision, digest, errors, the step as the gate reads it | Zero I/O, zero framework |
| `application/` | The use case: `evaluate_gate`, its one-poll bridge `evaluate_gate_sync`, the envelope step, and the request/ports a caller hands over | Engine services, `sha2` |
| `infrastructure/` | `GateSetup` — builds the two ports from `.rigorix/` for a native runtime | `toml`, filesystem, native only |
| `interfaces/` | **Not present, deliberately.** See below | — |

**Dependency rule:** `domain ← application ← infrastructure`. Nothing in `domain/` imports from the
other two.

**Why no `interfaces/`:** the gate exports no protocol. Its two callers *are* its interfaces — the CLI's
orchestrator (native, `cli/src/cli_boundary/orchestrator.rs`) and the crate's `EngineGate`
(`wasm/src/infrastructure/engine_gate.rs`, a host-supplied surface with `wasm-bindgen` exports on the
crate side). A fourth directory here would mirror a surface the engine does not own, which is the
duplication that `contracts/` wrappers were removed for.

## Components by Layer

### Domain Layer (`domain/`)

| Component | File | Description | Framework? |
|-----------|------|-------------|------------|
| `GateVerdict` | `verdict.rs` | `Dispatch` / `Promote { gate, rule_id, reason }` / `Refuse { … }`; `permits_dispatch()`, `awaits_approval()` | ❌ No |
| `GateOutcome` | `outcome.rs` | `Passed` / `Refused` / `Promoted` / `CouldNotEvaluate` — four values, because a run's outcome is not binary | ❌ No |
| `GateId` | `finding.rs` | `Preconditions` / `Requirements`, with their stable wire strings | ❌ No |
| `GateFinding` | `finding.rs` | One gate's conclusion, in evaluation order; redacted `detail`, opaque `evidence` | ❌ No |
| `GateDecision` | `decision.rs` | Verdict + ordered findings + envelope identity (+ signed evidence when a run context was supplied); equality is by what a verifier sees | ❌ No |
| `decision_digest` | `decision.rs` | SHA-256 over the canonical `{verdict, findings}` — the determinism claim as a value | `sha2` only |
| `GateEnvelopeBinding` | `binding.rs` | `execution_id`, `decision_digest`, pinned instant, `pinned` | ❌ No |
| `GateEnvelopeEvidence` | `binding.rs` | The signed envelope, its canonical bytes and its hash | audit domain types |
| `GateError` | `error.rs` | `UnpinnedExecution`, `Precondition`, `NotReady`, `Envelope`, `Requirements` — every variant a refusal at the caller | `thiserror` |
| `GatedStep` | `step.rs` | name, tool, parameters — exactly what the two services read, and nothing else | ❌ No |

### Application Layer (`application/`)

| Component | File | Description |
|-----------|------|-------------|
| `evaluate_gate` | `compose.rs` | The composition: preconditions → requirements, fail closed at the first refusal |
| `evaluate_gate_sync` | `sync.rs` | One poll of the same future; `GateError::NotReady` if it pends. Never a retry loop, never a panic (#1389) |
| `decision_evidence` | `envelope.rs` | Builds and signs the record from a `RunContext`; the engine assembles, the runtime holds the key |
| `GateRequest` | `request.rs` | step, proposed action, identity attestation, `PinnedInputs`, `GatePorts`, optional `RunContext` |
| `GatePorts` | `request.rs` | `Option<Arc<dyn PreconditionService>>` + `Option<Arc<dyn SequencePolicyService>>`; `None` means the operator already decided not to gate |
| `RunContext` | `request.rs` | The run facts a gate cannot derive (template, prompt, tokens, author, commit, key) — no `Default`, because a default inside a signature is a fabrication |

### Infrastructure Layer (`infrastructure/`)

| Component | File | Description |
|-----------|------|-------------|
| `GateSetup` | `setup.rs` | `from_env` reads `.rigorix/preconditions.toml` and the sequence policy, and arms both ports; native-only (`--gates` holds the inventory to this) |

## Data Flow

```
step + pins + ports
        │
        ▼
  evaluate_gate ──► precondition service ──► GateFinding(Preconditions, …)
        │                                         │
        │  refused / could not evaluate ──────────┴──► stop, fail closed
        ▼
     requirements ──► GateFinding(Requirements, …)  (Deny | Promote | unmet)
        │
        ▼
  GateDecision { verdict, findings, binding{ execution_id, decision_digest } }
        │
        └─► RunContext? ─► decision_evidence ─► signed envelope + canonical bytes + hash
```

The digest is computed over what the two services produced, in the order they produced it. The clock is
never read: the decision instant comes from `PinnedInputs`, and a request without an execution id is
rejected rather than defaulted (ADR-014 §2).

## Design Principles

1. **The engine decides, the runtime signs** (ADR-013). No key, no verifier, no envelope assembly in
   the composition path.
2. **Fail closed at the first refusal.** A later gate cannot un-refuse an earlier one.
3. **Three outcomes, not two.** A value that lands in a two-valued slot is absorbed by the permissive or
   terminal one: `Promote → Passed` was #1392, `Unknown → Failed` was #1393.
4. **Report, do not mutate.** The dispatch path consults `verdict.awaits_approval()`; a gate that wrote
   `requires_approval` into the plan would leave a stale flag on reuse.
5. **No domain type from a port.** `GatePorts` holds other contexts' *service* traits; the decision's
   language imports nothing from them.

## Degradation Strategy

| Condition | Behaviour |
|---|---|
| No preconditions file, no sequence policy | `Dispatch` with no findings — the operator's decision not to gate is a decision, not an error |
| A gate cannot evaluate | `Refuse` with `GateOutcome::CouldNotEvaluate`; never `Passed` |
| A port awaits on the sync path | `GateError::NotReady`, typed and actionable — a build defect, not a hang |
| No `RunContext` | Digest-only decision: a decision is still a decision without a record of who ran it |
| Signing key absent in `RunContext` | Unsigned envelope — recorded, not attested; a different thing from a forged signature |
| No execution id | `GateError::UnpinnedExecution` |

## Dependencies

**Inbound (this module uses):** `audit::application::dto::PinnedInputs`, `audit` envelope factory (for
the record step only), `precondition::application::service::PreconditionService`,
`sequence_policy::application::service::SequencePolicyService`, `configuration::domain::Secret`,
`identity::domain::IdentityRef`, `sha2`.

**Outbound (who calls this module):** `cli/src/cli_boundary/orchestrator.rs` (native, via
`infrastructure::GateSetup`), `wasm/src/infrastructure/engine_gate.rs` (the crate, supplying its own
ports), and `engine/src/precondition/application/setup.rs` (shares the precondition service instance so
both readers see the same operator file).

## Security Considerations

- Redaction at construction: `detail` never carries parameter values or stdout (`SpanPrivacy`).
- `RunContext::signing_key` is a `Secret`, so a `Debug` of a request cannot print a key.
- The digest covers no ambient value — no clock, no hash-map order, no path.
- No filesystem, no network, no process on the portable path; `infrastructure/` is where any of those
  would be, and it is `cfg(not(feature = "portable"))`.

## Error Handling

Every `GateError` variant is a refusal at the caller; none is retriable and none is swallowed.

| Variant | Cause | Caller's action |
|---|---|---|
| `UnpinnedExecution` | no execution id in `PinnedInputs` | pin one; do not invent one |
| `Precondition { detail }` | the precondition service failed | refuse the step |
| `NotReady` | a port awaited on the first poll | a wiring defect in the host's ports |
| `Envelope { detail }` | the factory refused the input or signing failed | refuse; do not dispatch unsigned |
| `Requirements { detail }` | the sequence policy failed | refuse the step |

## Acceptance Criteria

- `engine/src/gate/` has `domain/`, `application/` and `infrastructure/`; `check_ddd_structure.sh`
  passes for `gate` without an exemption.
- `evaluate_gate` and `evaluate_gate_sync` produce the same decision and the same digest for the same
  request (`both_paths_produce_the_same_bytes_and_the_same_digest`).
- The order is preconditions then requirements, with no second ordering in any caller
  (`the_order_is_preconditions_then_requirements`).
- `Promote` is reachable and is distinct from `Dispatch` and `Refuse`, and it changes the digest
  (`a_promotion_changes_the_decision_digest`).
- A port that awaits on the sync path yields `GateError::NotReady`, never a panic
  (`a_port_that_awaits_is_reported_not_ready`).

## Testing Requirements

```
cargo test -p rigorix-engine --test unit gate            # 26 tests
cargo test -p rigorix-engine --features portable --tests # 897 passed, 0 failed, 48 binaries
cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown
bash wasm/ci/run-wasm-gates.sh                           # 10/10
bash engine/.pi/scripts/ci/check_ddd_structure.sh        # gate — all layers present
```
