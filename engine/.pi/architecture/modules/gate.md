# Gate Architecture

<!--
Canonical Reference: .pi/architecture/modules/gate.md
ADR: .pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md#2
Epic: portable-slice (Phase C)
Issues: #1381 (C.1), consumed by #1382 (CLI) and #1383 (wasm crate)
-->

## Purpose

One function that decides whether a step may be dispatched, called by every runtime that
decides. The module owns **the order** in which the existing gates run, and nothing else.

## What is in it

| path | role | portable |
|---|---|---|
| `application/compose.rs` | `evaluate_gate` — the composition; `decision_digest` | yes |
| `application/dto.rs` | `GateRequest`, `GateDecision`, `GateFinding`, `GatePorts`, `GateError` | yes |
| `application/setup.rs` | `GateSetup::from_env` — reads `.rigorix/`, arms or fails closed | **no** (native composition root) |

`setup` is the only gated file, and the reason is a dependency, not a preference: it names
`PreconditionSetup` and `SequencePolicySetup`, which build process runners and TOML
repositories. The slice's hosts supply their ports directly, which is what makes the crate
able to call the same composition without carrying a filesystem.

## The order

```
1. preconditions  (ADR-017)                 may the step dispatch at all?
2. requirements   (ADR-013 R9 / ADR-015)    which obligations does it leave unmet?
```

Fail closed at the first refusal, and stop: a step that will never dispatch has no obligations
to satisfy, so asking would produce evidence about a run that cannot happen.

The order is asserted twice — once by an order-recording port
(`the_order_is_preconditions_then_requirements`) and once by a behaviour
(`a_refusal_precondition_is_not_displaced_by_a_requirement_promotion`). Swapping the two blocks
fails four tests.

## What it does not do

- **No domain.** A `GateFinding` is one of `precondition`'s or `sequence_policy`'s findings,
  re-labelled with the gate that produced it.
- **No configuration.** Absence of a gate is a decision the operator made, so `GatePorts::none()`
  dispatches with no findings; it is never an error.
- **No signing.** The engine publishes the decision's digest; the runtime that holds a key signs
  it. The wasm bundle carries no key and no verifier (ADR-013).
- **No defaults for identity.** `PinnedInputs::execution_id()` is required — `GateError::UnpinnedExecution`
  rather than a nil UUID, because a defaulted id makes two runs collide in one record.

## Consumers

| caller | how it calls |
|---|---|
| `cli` | `GateSetup::from_env` once; `execution_config` + `orchestrator_builder` attach both ports |
| `rigorix-gate-wasm` | `evaluate_gate` with host-supplied ports (Phase C.3, #1383) |

## Verification

```
cargo test -p rigorix-engine --test unit gatecomposition     9 tests
bash .pi/scripts/validate-portable-inventory.sh [--gates]   manifest agrees with the code
```
