---
guardian_issue:
  id: "ISSUE-DEFAULT-ENGINE-FLIP"
  title: "Flip `default = [\"engine\"]` — the one line that makes the shipped module able to decide, blocked on the wasm graph carrying no async runtime"
  epic: "portable-slice"
  component: "Wasm"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1401
  dependencies:
    - "#1400 (must be merged — the wasm32 dependency graph must carry no async runtime or timer)"

  intent: |
    `wasm/Cargo.toml` still ships `default = []`, so the artifact a host loads refuses at
    `open_session` with `gate_unavailable` and reports `"gate_evaluation": false`. Everything needed to
    make that false is merged: the engine's gate composition, the four host ports, the envelope
    assembly, the third outcome, and a size budget that measures the artifact under the size profile
    (**169 250 B shipped of a 1 500 000 B ceiling**).

    This issue exists on its own because it is the one change that turns the crate from a contract with
    a gate behind a flag into a gate. It was previously the last line of #1386, which is how #1386
    stayed open after its own work was finished — an issue whose exit depends on another open issue
    never closes, and reads as unfinished work.

  in_scope:
    - "`wasm/Cargo.toml`: `default = [\"engine\"]`. The feature's contents do not change; `engine` remains selectable off with `--no-default-features`, and that configuration keeps refusing at `open_session`"
    - "`wasm/ci/check-bundle-independence.sh` asserts the **property** instead of the crate name: the `tokio` entry in its `BANNED` list currently reads 'no async runtime in a single-threaded module', and after the flip the module legitimately carries `tokio` with `sync` only, so the rule as written would fail on the design. It must assert that the wasm32 graph's tokio feature set is a subset of `{sync}` and that the module contains no timer or spawn symbol"
    - "The wasm32 clippy run's three diagnostics are resolved or explicitly allowed, because the flip makes wasm32 the shipped artifact: `clippy.toml`'s `tokio::process::Command` disallowed-type is unresolvable on this target (it warns rather than enforcing), plus a pre-existing `default_timeout` dead field and its `Duration` import"
    - "The budget is re-recorded from the *flipped* artifact under the size profile, in the same PR: `measured_bytes` = that artifact, `budget_bytes` = +25 %, `ceiling_bytes` unchanged at 1 500 000"
    - "`contract_version()` on a default build reports `\"gate_evaluation\": true`, and `open_session` succeeds"
    - "The `--no-default-features` path keeps its test: `gate_evaluation: false`, `open_session` refuses with `gate_unavailable`"

  out_of_scope:
    - "The size work — merged, with `ci/wasm-build.sh` owning the profile and `ACTUAL == MEASURED` asserted"
    - "The conformance flip (`gate: \"cli\"`) — #1385, which becomes unblocked by this issue and must land after it"
    - "Changing the ceiling. 1 337 205 B was measured before the budget re-record; if the flipped artifact exceeds 1 500 000 B, that is a finding to file"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "`wasm/Cargo.toml` — the default feature list"
      - "`wasm/ci/bundle-budget.txt` — re-recorded from the flipped artifact"
    interfaces:
      - "`contract_version()` capability report on a default build"

  citations:
    - file: "wasm/Cargo.toml"
      note: ":16 `default = []` — the line this issue changes"
    - file: "wasm/src/interfaces/wasm_api.rs"
      note: ":93 `gate_evaluation` in the capability report; `:302` `cfg!(feature = \"engine\")`"
    - file: "wasm/ci/bundle-budget.txt"
      note: "measured 169 250 / budget 211 562 / ceiling 1 500 000 — the numbers the flip re-records"
    - file: "wasm/ci/check-bundle-independence.sh"
      note: "the check that refuses an async runtime in the module (#1400)"

  verification:
    - "`cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm` (no feature flags, via `ci/wasm-build.sh`) produces a module whose `contract_version()` reports `\"gate_evaluation\": true`"
    - "`node wasm/ci/module-smoke.mjs <glue> …` passes with a **decision**, not a `gate_unavailable` refusal"
    - "Seed: remove `engine` from `default` → the smoke gate's decision assertion fails naming the refusal, proving the flip is what changed the behaviour"
    - "`cargo build --no-default-features` still yields `gate_evaluation: false` and an `open_session` refusal"
    - "`bash wasm/ci/run-wasm-gates.sh` is 10/10, and `check-bundle-size.sh` prints `actual` equal to the newly recorded `measured_bytes`"
---

# One line, and the epic's whole purpose

Everything else in this epic exists so that this line can be flipped with a measurement behind it
rather than a hope. Keeping it as the last line of a size issue made a finished issue look unfinished;
keeping it here makes one line the subject of one issue.
