---
guardian_issue:
  id: "ISSUE-WASM-BUNDLE-GATES-MEASURE-DEBUG-ARTIFACT"
  title: "The wasm size gates measure an artifact no host ships: 341 785 B of the engine-linked bundle is a debug name section, and 33 485 B of the current budget's subject is the same"
  epic: "portable-slice"
  component: "Wasm"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1353
  dependencies:
    - "#1351 (the spike that produced the measurement this issue corrects)"

  intent: |
    `wasm/ci/check-bundle-size.sh` measures the bytes `wasm-bindgen` emits, and
    `wasm/ci/bundle-budget.txt` records that number as the artifact CI ships. It is not: the
    artifact carries a **custom `name` section** — the wasm debug symbol table — which no
    production host needs and which a trimming step removes. On the empty seam that section is
    33 485 B of 243 366 B (13.8 %); with the engine linked it is **341 785 B of 1 512 420 B
    (22.6 %)**.

    The consequence is not cosmetic. The spike's branch C fired on that number: "the bundle
    does not fit" — a verdict about a debug-profile artifact — and the decided next step was to
    narrow the demo to verify-only. Measured on the artifact a host actually loads, the
    engine-linked bundle is **1 170 635 B, 78.0 % of the 1 500 000 B ceiling**, and it passes all
    fifteen `module-smoke.mjs` assertions identically to the unstripped build.

    A size gate whose subject is not the deliverable is the same defect as #1349 (ten gates
    passing on a 368-byte module with no exports), one layer up: the gate is real, the number is
    real, and the artifact is not the thing anyone ships. The budget file's own note even
    claims a step that never ran — "wasm-bindgen output after wasm-opt" — while `wasm-opt` is
    absent from this machine and from CI.

  in_scope:
    - "A deterministic section-level strip of the `name` custom section, run by the packaging path itself, with no binaryen dependency (`wasm-opt` is absent here and in CI)"
    - "The size gate measures the stripped artifact; `bundle-budget.txt` is re-recorded from it (measured 209 881 B, budget measured + 25 % = 262 351 B) and its note stops claiming a wasm-opt pass"
    - "A presence assertion: the packaged artifact must contain no `name` custom section, mirroring the size assertion so that skipping the strip fails the gate on its own"
    - "The same stripped artifact is what `module-smoke.mjs` and `run-host-verification.sh` load, so the number and the smoke subject are one artifact"

  out_of_scope:
    - "Removing `regex` from the slice. That experiment was the branch table's named trim; it is unnecessary once the ship artifact is measured, and it stays unrun until a size problem exists on the shipped artifact"
    - "Adding `wasm-opt`/binaryen to CI. The strip must be reproducible without it"
    - "Re-recording the budget for the engine-linked build. That number belongs to #1351's M3 re-run, not to this gate's subject"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "The packaging step in wasm/ci/run-wasm-gates.sh (or the strip helper it calls)"
      - "wasm/ci/check-bundle-size.sh — subject changes from emitted bytes to shipped bytes"
      - "wasm/ci/bundle-budget.txt — measured_bytes, budget_bytes, note"
    interfaces:
      - "None — the exported surface is unchanged; the stripped module was verified against all fifteen smoke assertions"

  citations:
    - file: "wasm/ci/bundle-budget.txt"
      note: "measured_bytes: 243366, budget_bytes: 304207, and the note claiming `after wasm-opt`"
    - file: "wasm/ci/check-bundle-size.sh"
      note: ":45-48 reads measured_bytes/budget_bytes from the budget file — the gate enforces whatever number the file holds"
    - file: "wasm/ci/module-smoke.mjs"
      note: "the fifteen assertions the stripped artifact must keep passing"
    - file: "wasm/ci/run-wasm-gates.sh"
      note: "the stage order the strip must join, before the smoke stage loads the module"

  verification:
    - "`bash wasm/ci/run-wasm-gates.sh` exits 0, and the printed measured size is the stripped byte count (209 881 B for the current empty seam, not 243 366 B)"
    - "The packaged artifact contains no custom section named `name`: a `grep -a -c 'name'` is not the check — the gate parses sections and asserts on the section id/name pair, and a fixture binary carrying a name section fails it"
    - "Seed: skip the strip step → the presence assertion fails with exit 1 and names the artifact; the size assertion alone would still pass, which is why the presence assertion exists"
    - "`node wasm/ci/module-smoke.mjs <glue> attempt_run,contract_version,export_artifact_set,open_session` exits 0 against the stripped artifact — verified today: 15/15 pass, identical to the unstripped run"
    - "`bash wasm/ci/run-host-verification.sh` exits 0 against the stripped artifact"
    - "The budget note contains no claim about wasm-opt: `grep -c 'after wasm-opt' wasm/ci/bundle-budget.txt` returns 0"
---

# The size gates measure a debug artifact

## Measured today

All four numbers reproduce bit-exactly; the commands are in the reproduction block.

| artifact | emitted bytes | `name` section | stripped bytes |
|---|---|---|---|
| empty seam (default profile) | **243 366** — matches the committed `measured_bytes` exactly | 33 485 (13.8 %) | **209 881** |
| engine linked (LTO=fat, opt-level=z, panic=abort, codegen-units=1) | **1 512 420** — matches the spike report exactly | 341 785 (22.6 %) | **1 170 635** = 78.0 % of the 1 500 000 ceiling |

Engine-linked section sizes: `code` 763 164 B (50.5 %), `data` 400 735 B (26.5 %), `custom`
341 781 B (22.6 %). The other custom sections are `target_features` (148 B) and `producers`
(112 B); they stay — `target_features` is meaningful to some runtimes and neither is a symbol
table.

The stripped engine-linked module passes the crate's own smoke gate unchanged:

```
✓ module loaded        ✓ attempt_run()        ✓ contract_version()      ✓ export_artifact_set()
✓ open_session()       ✓ no unannounced export ✓ contract_version 1.12.1 ✓ versions agree
✓ artifact_format 1    ✓ no verdict-shaped field ✓ opaque handle        ✓ refusal kind: seal_failed
✓ gate_unavailable     ✓ malformed handle throws
✅ the module exports the contract and answers all four functions     EXIT=0
```

## Reproduction

```sh
# empty seam — reproduces the committed 243 366 exactly
cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm
wasm-bindgen --target nodejs --out-dir /tmp/pkg target/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm
wc -c < /tmp/pkg/rigorix_gate_wasm_bg.wasm                                  # 243366

# engine linked, trimmed — reproduces the spike report's 1 512 420 exactly
CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
CARGO_PROFILE_RELEASE_PANIC=abort CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm --features engine
wasm-bindgen --target nodejs --out-dir /tmp/pkg-z target/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm
wc -c < /tmp/pkg-z/rigorix_gate_wasm_bg.wasm                                # 1512420
```

The strip is a section walk — 8-byte header, then `id | LEB128 size | body` — dropping only
`id == 0` whose name is `name`. No tool beyond a shell and one small script.

## Why the presence assertion is the load-bearing one

With `measured_bytes: 209881` and `budget_bytes: 262351`, an *unstripped* 243 366 B artifact still
sits under budget and the size gate passes. Size alone therefore cannot prove the strip ran. The
assertion that can is "this artifact carries no `name` section", and it has to be shown to fire:
skip the strip, run the gates, and the presence check must exit 1 and name the artifact.

## What this issue is not

It does not re-open the regex question, and it does not re-record the engine-linked budget. It
fixes the subject: a size gate is only a gate when the bytes it counts are the bytes a host loads.
