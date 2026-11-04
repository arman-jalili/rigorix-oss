---
guardian_issue:
  id: "ISSUE-PHASEC-1-ENGINE-GATE-ENTRY-POINT"
  title: "Phase C.1 — the engine gains one gate entry point that takes ports, so two runtimes can call the same composition"
  epic: "portable-slice"
  component: "Engine"
  module: "gate-composition"
  status: planned
  priority: high
  tracking_issue: 1381
  dependencies: []

  intent: |
    The composition of a gate decision does not live in the engine. It lives in
    `cli/src/cli_boundary/orchestrator.rs:338`, while the two parts it composes are separate engine
    entries: `PreconditionDispatchGate::new(Box<dyn PreconditionService>)`
    (`precondition/application/gate_impl.rs:42`) and the sequence-policy service that calls
    `req.evaluate(&step.name, &step.tool, &step.parameters, …)`
    (`sequence_policy/application/service_impl.rs:335`).

    So any second runtime that wants to decide has exactly two options: call those two itself and
    reproduce the CLI's ordering, or don't. The first makes "the browser decides the same as the
    engine" a coincidence maintained by two callers; the second makes it false.

    This issue adds the missing third option: one engine function both callers call.

  in_scope:
    - "One public entry point in a composition module (`engine/src/gate/`, `application/` layer) whose module doc states it owns no domain and composes `precondition` and `sequence_policy` — the ordering lives here and nowhere else"
    - "The request type carries exactly three things: the step and its proposed action, the pinned inputs (`PinnedInputs`, ADR-014 §2), and the ports as injected trait objects (`Box<dyn PreconditionService>`, the provider transport, the key source)"
    - "The decision type carries the ordered findings and the `BuildEnvelopeInput` the audit factory consumes — so a caller signs a decision rather than assembling one"
    - "The CLI's composition at `orchestrator.rs:338` is rewritten to call it, with the CLI's existing tests unmodified as the proof that behaviour did not move"
    - "Registration: ADR-014 §2 records the entry point as the decision the phase rests on; a module doc under `engine/.pi/architecture/modules/` if the architecture guards require one for a new top-level module"

  out_of_scope:
    - "A `Clock`/`IdSource` trait pair. ADR-014 §2 removed it: `PinnedInputs` is the carrier, one site, measured"
    - "Changing how findings are produced. The order and the semantics stay exactly what the CLI does today; this issue relocates the composition, it does not redesign it"
    - "The wasm crate, conformance and the demo — Phase C.3 onward"
    - "Any behaviour change to a decision: a plan that decided X before this issue decides X after it, and the CLI's suite is the evidence"

  affected_layers:
    domain:
      - "None — no new domain type; the request and decision are application DTOs"
    application:
      - "New composition module; `sequence_policy` and `precondition` are consumed, not modified"
      - "`cli/src/cli_boundary/orchestrator.rs` — composition replaced by the call"
    infrastructure:
      - "None"
    interfaces:
      - "None"

  citations:
    - file: "engine/src/precondition/application/gate_impl.rs"
      note: ":26 `PreconditionDispatchGate`, :42 `new(Box<dyn PreconditionService>)` — ports already injected"
    - file: "engine/src/sequence_policy/application/service_impl.rs"
      note: ":335 `req.evaluate(…)` — the requirement side of the composition"
    - file: "cli/src/cli_boundary/orchestrator.rs"
      note: ":338 and :700 build `sequence_policy`; :418 wires it — the composition that must move"
    - file: "wasm/src/domain/ports/mod.rs"
      note: ":46 `Ports { clock, ids, randomness, key_source, provider }` — the ports a host will supply"
    - file: "wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md"
      note: "§2 — `PinnedInputs` through an additive method is the determinism decision"

  verification:
    - "`cargo test -p rigorix-engine -p rigorix-cli` passes with **zero test edits** — the same standard M1b held the `portable` feature to"
    - "`grep -c 'sequence_policy' cli/src/cli_boundary/orchestrator.rs` returns 0: the composition is no longer in the CLI"
    - "One test calls the entry point directly with a fixture port set and asserts a decision carrying findings and an envelope input; a second asserts the same decision for the same input twice (determinism at the entry point, not in a caller)"
    - "Seed: swap two lines in the composition (evaluate requirements before preconditions) → a test fails naming the ordering, proving the order is asserted rather than incidental"
    - "Seed: drop one port from the request type → compile error, not a silent default — the ports are required, not optional"
---
**This issue is C.1 (#1381). Prerequisite: none — this is the first.** Do not begin work before every
prerequisite listed above is *merged* — a locally patched engine or a stubbed crate makes
this issue's acceptance criteria untestable.

**The Phase C chain, in order:**

```
C.1 #1381  engine gate entry point        (no prerequisite)
C.2 #1382  CLI adopts it                 needs C.1
C.3 #1383  crate wires EngineGate        needs C.1, C.2   <- do not start before both are merged
C.4 #1384  gate_unavailable narrowed     needs C.3
C.5 #1385  conformance -> gate: "cli"    needs C.3, C.4
C.6 #1386  size budget re-recorded       needs C.3, C.4, C.5
C.7 #1387  spec records the evidence     needs C.4, C.5
```


# Before you start

**This issue precedes Phase C.3 (the crate wiring) and must be merged first.** The crate's acceptance
criterion is that it calls *the same function* the CLI calls. Wiring the crate against a locally
patched engine makes that criterion untestable, and a worker who starts at C.3 will end up composing
engine parts inside the crate — the outcome this issue exists to prevent.

# Why it is the engine's job

The two halves of a gate decision are engine code; the order they run in is not. Whoever holds the
order defines the decision. Today that is the CLI, so a browser build cannot be equal to it by
construction — only by imitation.
