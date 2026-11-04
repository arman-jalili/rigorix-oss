---
guardian_issue:
  id: "ISSUE-PHASEC-3-CRATE-WIRES-ENGINEGATE"
  title: "Phase C.3 — `EngineGate` calls the engine's gate entry point; `SeamGate::not_linked()` is deleted and the crate composes nothing"
  epic: "portable-slice"
  component: "Wasm"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1383
  dependencies:
    - "Phase C.1 — the engine gate entry point (must be merged)"
    - "Phase C.2 — the CLI calls it (must be merged)"
    - "Phase C.1b #1389 — the sync façade and engine-owned envelope assembly (must be merged)"

  intent: |
    The crate already has the carrier: `GateEvaluator::evaluate` takes a `GateEvaluationRequest` and
    returns `EngineEvaluation { findings, envelopes, port_failures }`
    (`wasm/src/domain/engine_seam.rs`). Today the only implementation is
    `SeamGate::not_linked()`, which refuses every run with `gate_unavailable`.

    This issue replaces it with an implementation that calls the engine's single gate entry point
    (Phase C.1) and maps what comes back into the carrier. The crate does not call two engine
    functions, does not order them, and does not decide anything the engine did not decide — that is
    the whole point of C.1 existing.

  in_scope:
    - "`EngineGate` implements `GateEvaluator` by calling `evaluate_gate_sync` once, with the feature `engine` on; it does not drive, poll or assemble anything itself"
    - "The crate supplies the three narrow adapters the manifest excludes — the precondition **runner**, the **repository** (rules as data) and the **provider client** (remote determinations) — and the engine's `PreconditionServiceImpl` still decides. **Correction to the first draft of this issue: the crate does not supply a `PreconditionService`.** The manifest excludes *items inside* `service_impl` (`2` sites: the default wiring that names `HttpProviderClient`), not the module, so the evaluator stays in the slice and the crate must not re-implement it"
    - "Fixture determinations speak the **crate port's** vocabulary — `Determination` in `wasm/src/domain/ports/provider_transport.rs`, serde form `\"passed\" | \"failed\" | \"error\"` — because the fixture is host input to the crate, not engine input. The crate converts once, in one exhaustive `From`, into the engine's `PreconditionOutcome` (same three words; #1391 makes the identity a test). A missing determination for a *matching* precondition maps to the third value, which refuses: absence is never `passed`"
    - "`NotReached` never comes from a fixture. It is the crate's `StepDecision` value meaning *the gate did not evaluate this step*, produced from the run's step ordering, not from any determination — a host that could assert what the gate did not do would invert the producer/verifier split"
    - "Ports are satisfied from the crate's `Ports` struct: `provider` implements the engine's provider port, `key_source` signs, and `PinnedInputs` supplies the instant and the ids — no port is left to a default that could silently answer"
    - "A port that cannot answer produces an entry in `port_failures`; the aggregate refuses on a non-empty list even when every finding says `Passed` (invariant 2, already enforced in the domain)"
    - "`SeamGate::not_linked()` and its `gate_unavailable` path for a linked build are deleted; `mod.rs` wiring picks `EngineGate` whenever the feature is on"
    - "The contract tests that asserted `gate_unavailable` now assert a real decision; the seeded-divergence and malformed-handle assertions are unchanged"

  out_of_scope:
    - "Inventing the composition, the port mapping or the envelope input — C.1 and C.1b supply all three. If the shape does not fit the carrier, stop and file the delta; do not work around it inside the crate"
    - "A portable precondition evaluator inside the engine. That would put fixture logic on the production path and make `portable` mean 'the engine with stand-in checks' — the stub problem the inventory's `replaces-with` column exists to make visible"
    - "Conformance (`gate: \"cli\"`) — Phase C.5"
    - "The size budget — Phase C.6"
    - "Any engine type re-modelled in the crate. The carrier is opaque on purpose (ADR-013): findings and envelopes cross, nothing else"

  affected_layers:
    domain:
      - "None — `EngineEvaluation` already exists and is unchanged"
    application:
      - "`wasm/src/infrastructure/engine_gate.rs` — the adapter"
    infrastructure:
      - "`wasm/Cargo.toml` — the `engine` feature gains `dep:rigorix-engine` and `rigorix-engine/portable`"
    interfaces:
      - "`attempt_run` routes through the wired gate; the four exported functions are unchanged"

  citations:
    - file: "wasm/src/domain/engine_seam.rs"
      note: "`GateEvaluator`, `GateEvaluationRequest`, `EngineEvaluation`, `EngineFinding` — the carrier"
    - file: "wasm/src/infrastructure/seam_gate.rs"
      note: "`not_linked()` — the refusal this issue deletes"
    - file: "wasm/src/domain/ports/mod.rs"
      note: ":46 `Ports` — `provider` is the same role as the engine's provider port"
    - file: "wasm/ci/module-smoke.mjs"
      note: "the assertion `the unlinked seam refuses with gate_unavailable` — flips to a real decision"

  verification:
    - "`bash wasm/ci/run-wasm-gates.sh` exits 0, and the smoke gate asserts a **decision**, not a refusal: a seeded action that must be refused is refused with its evidence, and one that must pass passes"
    - "Seed: make the provider port answer `passed` for a fixture that says `failed` → the crate's decision changes, proving the port is consulted rather than assumed"
    - "Seed: return a non-empty `port_failures` with all findings `Passed` → the aggregate refuses (invariant 2 fires)"
    - "`cargo check -p rigorix-gate-wasm --target wasm32-unknown-unknown --features engine` exits 0 and the bundle still carries no verifier (the architecture test from ADR-013 stays green)"
    - "Two calls with the same pinned inputs produce identical envelope bytes — the witness `c47b27f9…`-class check, now produced by the crate rather than by an example"
    - "The same fixture world through the CLI (once `--fixture-world` lands in C.5) produces the same envelope bytes as the crate — the comparison C.5 then labels `gate: \"cli\"`"
---
**This issue is C.3 (#1383). Prerequisite: C.1 (#1381), C.2 (#1382) and C.1b (#1389).** Do not begin work before every
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

**Phase C.1 and C.2 must be merged.** This issue is deliberately the third: it consumes an entry point
that owns the ordering. If you find yourself calling `PreconditionDispatchGate` and a
`sequence_policy` service from the crate, stop — that is the composition C.1 moved, and reproducing it
here would make the conformance claim in C.5 a coincidence.
