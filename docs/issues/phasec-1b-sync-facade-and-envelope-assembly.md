---
guardian_issue:
  id: "ISSUE-PHASEC-1B-SYNC-FACADE-AND-ENVELOPE-ASSEMBLY"
  title: "Phase C.1b — a synchronous façade over the async gate, and envelope assembly owned by the engine so the runtime does not become the second composition"
  epic: "portable-slice"
  component: "Engine"
  module: "gate-composition"
  status: planned
  priority: high
  tracking_issue: 1389
  dependencies:
    - "#1381 (the async composition this builds on)"
    - "#1382 (the CLI caller that will also adopt the envelope step)"

  intent: |
    Two facts from C.1/C.2 block C.3, and neither is a wiring problem.

    **1. The carrier is synchronous by frozen contract; the composition is async.**
    `GateEvaluator::evaluate` is sync (`wasm32` has no executor), `evaluate_gate` is
    `async` because `PreconditionService::evaluate` is `async fn`
    (`precondition/application/service.rs:52`). Something must bridge, and where it bridges
    decides whether the property is testable.

    **2. The decision is not yet evidence.** `GateDecision` carries `decision_digest` over
    `{verdict, findings}` and leaves the envelope to the caller. That is right — a gate knows
    nothing of templates, prompts or token totals, and filling a `BuildEnvelopeInput` with
    guesses would write fiction into signed evidence. But it moves the composition boundary
    rather than removing it: **assembling the envelope is now a second composition**, and if
    the CLI and the crate each assemble it, the two runtimes diverge one step after the step
    C.1 unified. The digest would match and the signed bytes would not.

  in_scope:
    - "`gate::application` gains `evaluate_gate_sync(&GateRequest) -> Result<GateDecision, GateError>`: it drives the async composition with a single poll, and a future that is not immediately ready returns a typed `GateError::NotReady` — never a panic (a panic is a build defect on wasm32, ADR-012)"
    - "The property behind it is asserted, not assumed: a test polls every port implementation the slice can construct and asserts the first poll is `Ready`. The fixture-backed services the host supplies must satisfy it"
    - "Envelope assembly joins the entry point: `GateRequest` gains `run_context: Option<RunContext>` carrying exactly the fields a gate cannot derive (template id, prompt, source/repository, author, token totals, duration, commit/branch); when present, `GateDecision` carries the signed envelope built by `AuditEnvelopeFactoryImpl` with `PinnedInputs`, keyed by the supplied key source. When `None`, the decision carries the digest alone, exactly as today"
    - "The CLI adopts the context path in the same PR, so both runtimes assemble envelopes through one function — zero test edits, as #1382 required"

  out_of_scope:
    - "Making `evaluate_gate` synchronous. The async services are the engine's; the façade is the bridge, and changing the traits would rewrite every caller"
    - "Putting an executor in the wasm crate. `block_on` in an adapter hides the readiness property in the layer that cannot test it"
    - "Inventing run context in the engine. The type requires what the caller has; a default is a fabrication that lands in a signature"
    - "Any change to `GateVerdict`, `GateFinding` or the ordering — C.1's 9 tests are the contract"

  affected_layers:
    domain:
      - "None"
    application:
      - "`engine/src/gate/application/` — the façade and the envelope step; `RunContext` DTO"
      - "`cli/src/cli_boundary/` — the context path"
    infrastructure:
      - "None"
    interfaces:
      - "None — the wasm crate's sync trait is unchanged, which is the point"

  citations:
    - file: "engine/src/gate/application/compose.rs"
      note: ":46 `pub async fn evaluate_gate(&GateRequest) -> Result<GateDecision, GateError>`"
    - file: "engine/src/gate/application/dto.rs"
      note: ":66 `GatePorts { precondition: Option<Arc<dyn PreconditionService>>, sequence: … }`; :211 `GateEnvelopeBinding`; :231 `GateDecision`; :254 `GateError`"
    - file: "engine/src/precondition/application/service.rs"
      note: ":43 `PreconditionService: Send + Sync` with `async fn evaluate` as the reason the composition is async"
    - file: "engine/src/audit/application/envelope_factory_impl.rs"
      note: "`with_pinned_inputs` — the factory the envelope step calls"

  verification:
    - "`evaluate_gate_sync` returns the same `GateDecision` as `block_on(evaluate_gate(…))` for the same request: a test asserts equality of verdict, findings and `decision_digest`"
    - "Seed: a port implementation whose `evaluate` awaits a `tokio::time::sleep(Duration::ZERO)` → `evaluate_gate_sync` returns `GateError::NotReady`, not a hang and not a panic"
    - "Seed: remove the run context → the decision carries the digest and no envelope; with it, the envelope verifies against the key that signed it"
    - "Both runtimes produce the same envelope bytes for the same request: the CLI path and the façade path compared in one test"
    - "`cargo test -p rigorix-engine -p rigorix-cli` passes with zero test edits; C.1's 9 tests and the ordering seeds are untouched"
---

**This issue is C.1b (#1389). Prerequisite: C.1 (#1381) and C.2 (#1382), both merged.**
**C.3 (#1383) now depends on this issue** — it cannot call an async composition from a sync trait, and it must not assemble envelopes itself.

**The Phase C chain, in order:**

```
C.1  #1381  engine gate entry point (async)   (no prerequisite)
C.2  #1382  CLI adopts it                      needs C.1
C.1b #1389  sync facade + envelope assembly    needs C.1, C.2  <- C.3 waits on this
C.3  #1383  crate wires EngineGate             needs C.1, C.2, C.1b
C.4  #1384  gate_unavailable narrowed          needs C.3
C.5  #1385  conformance -> gate: "cli"         needs C.3, C.4
C.6  #1386  size budget re-recorded            needs C.3, C.4, C.5
C.7  #1387  spec records the evidence          needs C.4, C.5
```

# Why both halves are one issue

They are the same failure seen twice: **a second composition appearing one layer out**. The façade is
where the sync/async seam is decided, and the envelope step is where "the engine decides, the runtime
signs" would otherwise become "the engine decides, and both runtimes sign differently". Fixing one
without the other leaves the other as the next adapter's job.

# Why a typed `NotReady` and not a retry loop

A retry loop is a busy-wait on `wasm32` with no executor behind it. A panic is forbidden (ADR-012).
The honest third option is a value: if the engine's gate path ever awaits real I/O in the slice, the
caller is told, and the test that polls every slice port fails first — at the place where the property
is defined.
