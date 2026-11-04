Phase C's first two links. The chain is seven issues; this PR closes the two that make the rest possible, and deliberately not the five that depend on a decision recorded below.

## #1381 — the engine owns the order

`engine/src/gate/application/compose.rs::evaluate_gate` composes the two gates:

```
1. preconditions  (ADR-017)                may the step dispatch at all?
2. requirements   (ADR-013 R9 / ADR-015)   which obligations does it leave unmet?
```

Fail closed at the first refusal, and stop — a step that will never dispatch has no obligations to satisfy, and asking anyway produces evidence about a run that cannot happen.

Three things the implementation **refuses** to invent, each because the alternative puts a guess into signed evidence:

| temptation | what was done instead |
|---|---|
| mint or default an execution id | `GateError::UnpinnedExecution` — a nil-UUID default makes two runs collide in one record |
| build a `BuildEnvelopeInput` because the issue names one | publish `decision_digest` (SHA-256 over `[verdict, findings]`); that type describes a *run* — template, prompt, token totals, commit — and a gate knows none of it |
| put the pins inside the digest | pins travel beside it, so two decisions at different instants are the same decision and a verifier replays with the recorded instant |

`GateSetup::from_env` is the composition root: the one place that knows a malformed `preconditions.toml` arms a fail-closed gate rather than none. It is gated out of the portable slice (the slice takes ports from the host), which is the 23rd gated item.

## #1382 — the CLI adopts it, and no test moved

```
grep -c 'sequence_policy' cli/src/cli_boundary/orchestrator.rs   →  0
git diff --stat cli/tests cli/src/cli_boundary/tests.rs          →  (empty)
cargo test -p rigorix-cli                                        →  98 + 18 + 1 + 2 passing
```

125 lines deleted, 47 added. That ratio is what C.1 buys: the composition moved into the engine, and behaviour did not move at all.

## Evidence

```
cargo test -p rigorix-engine -p rigorix-cli          3054 passed, 0 failed
cargo test -p rigorix-engine --features portable     889 passed  (was 880)
cargo test -p rigorix-engine --test unit gatecomposition   9 passed
cargo fmt --all --check                              clean
cargo clippy --workspace --all-targets -D warnings   0
validate-portable-inventory.sh [--gates]             green, self-test 17/17
docs guards ci canonical architecture tests script-drift shell-exit-status   exit 0
```

Seeds that fail when the defect is present:

- **Swap the two composition blocks** → four tests fail, two naming the order:
  `the_order_is_preconditions_then_requirements` and
  `a_refusal_precondition_is_not_displaced_by_a_requirement_promotion`.
- **A new top-level module that stays in the slice** → the inventory guard named the drift by itself: `22 of 38` → `22 of 39`, `22 items` → `23`, across the manifest summary, ADR-014 §1, the evidence table and the ratios file. Four self-test seeds had to be re-derived to the new counts, which is the ratchet doing what #1377 built it to do.

## Why this PR stops at C.2

C.3 (`EngineGate` calls this entry point) needs one decision this PR cannot make on its own, and guessing it would be worse than stopping:

`GateEvaluator::evaluate` is **synchronous** by frozen contract ("the wasm target has no async runtime"), while `PreconditionService` and `SequencePolicyService` are `async_trait`. Plus, the browser has no `PreconditionService` implementation to inject — the native one spawns processes against `.rigorix/preconditions.toml`, and `precondition::application::service_impl` is outside the portable slice (because it names an HTTP provider client).

So C.3 is not just wiring: it is either (a) a fixture-backed `PreconditionService` the browser supplies, called through `futures::executor::block_on`, or (b) an engine-side sync façade over the async services. Both are architectural choices with consequences for what the browser can actually evaluate, and both belong in a recorded decision before code. C.3's own text says as much: *"If the entry point's shape does not fit the carrier, stop and file the delta."*

Filed as the delta in the PR discussion rather than made unilaterally.

Closes #1381
Closes #1382
