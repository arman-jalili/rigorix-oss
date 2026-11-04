---
guardian_issue:
  id: "ISSUE-PHASEC-4-GATE-UNAVAILABLE-VOCABULARY"
  title: "Phase C.4 — `gate_unavailable` means 'this build cannot evaluate' and nothing else; the engine becomes the default build, the capability is published"
  epic: "portable-slice"
  component: "Wasm"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1384
  dependencies:
    - "Phase C.3 — the crate evaluates (must be merged)"

  intent: |
    Three different situations currently share one label:

    | situation | what a host must do |
    |---|---|
    | this build cannot evaluate | get a different build |
    | the engine ran and refused | change the input |
    | a port did not answer | supply the missing fixture |

    `gate_unavailable` covers all three today because the only implementation refuses with it. After
    C.3 the crate can decide, so the label must be narrowed rather than reused. The rule this issue
    installs:

    - `gate_unavailable` **only** when the build was compiled without evaluation — a capability
      absence, knowable before a session exists, so it refuses at `open_session`, not at
      `attempt_run`;
    - a real decision is a decision, with findings and evidence;
    - a port that could not answer keeps the existing `(kind, Some(port))` pairing that
      `PreconditionDispatchGate` errors already use (`interfaces/errors.rs`, `E::PortFailure`).

    And the shipped crate gets the engine **by default**, because an artifact whose default
    configuration cannot decide is not a gate. `portable` stays non-default in the engine; `engine` on
    the wasm crate becomes default.

  in_scope:
    - "`contract_version()` gains a capability report: `{\"gate_evaluation\": true|false}` — a host learns the build's capability before opening a session instead of inferring it from a refusal"
    - "`gate_unavailable` is emitted only by a build without evaluation, at `open_session`; `attempt_run` never returns it on a build that can evaluate"
    - "`wasm/Cargo.toml`: the `engine` feature becomes default (`default = [\"engine\"]`); the size gate therefore measures the real artifact"
    - "The vocabulary is recorded where hosts read it: the refusal table, the runbook's known-limits row, and the spec mapping (Phase C.7)"

  out_of_scope:
    - "Retiring the string. A dev build without evaluation still cannot decide, and a panic or an `allowed` would be worse lies than a refusal"
    - "Adding a new kind for runtime engine failure. The port pairing already distinguishes it; a new kind would duplicate that distinction in a second vocabulary"
    - "The npm publication path (separate decision: credentials and scope)"

  affected_layers:
    domain:
      - "`RefusalVocabulary` / the capability report — an accessor, no new error variant"
    application:
      - "`attempt_run`'s refusal path for a build without evaluation moves to `open_session`"
    infrastructure:
      - "`wasm/Cargo.toml` — `default = [\"engine\"]`"
    interfaces:
      - "`contract_version()` response gains `gate_evaluation`"

  citations:
    - file: "wasm/src/interfaces/errors.rs"
      note: ":21 `KIND_GATE`, and the `(kind, Some(port))` pairing used for `E::PortFailure`"
    - file: "wasm/src/interfaces/wasm_api.rs"
      note: "`contract_version`, `open_session`, `attempt_run` — where the refusal is emitted today"
    - file: "wasm/ci/module-smoke.mjs"
      note: "the `gate_unavailable` assertion that must flip"
    - file: "wasm/.pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md"
      note: "the tiers; this issue adds the vocabulary decision to them"

  verification:
    - "Default build: `cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm` produces a module whose `contract_version()` reports `\"gate_evaluation\": true`, and `open_session` succeeds"
    - "Build without the feature: `--no-default-features` → `contract_version()` reports `false`, and `open_session` refuses with `gate_unavailable`; `attempt_run` is unreachable because no handle exists"
    - "Seed: emit `gate_unavailable` from `attempt_run` on an evaluating build → a contract test fails naming the conflated case"
    - "Seed: a port that cannot answer → the refusal carries `port`, and its kind is not `gate_unavailable`"
    - "The size gate measures the default artifact (the wiring is no longer opt-in), and the presence assertion still fires when the strip is skipped"
---
**This issue is C.4 (#1384). Prerequisite: C.3 (#1383).** Do not begin work before every
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

**Phase C.3 must be merged.** Making `engine` the default before the crate can evaluate would make
every session refuse at open — a strictly worse artifact than today's, and a green test suite that
proves nothing. The order is: evaluate, then become the default, then narrow the label.
