---
guardian_issue:
  id: "ISSUE-WASM-GATES-PASS-ON-A-MODULE-THAT-EXPORTS-NOTHING"
  title: "every wasm gate passes on a 368-byte module with no exported functions, so nothing would notice if the packaging never landed"
  epic: "ci-integrity"
  component: "CI"
  module: "wasm-gates"
  status: planned
  priority: high
  tracking_issue: 1349
  dependencies:
    - "#1346 — the crate is not on the release train either"

  intent: |
    The crate's deliverable is a module a host can call. Measured today:

    ```
    $ cargo build --release --target wasm32-unknown-unknown      → ok          (gate: "release build for wasm")
    $ ls -l target/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm
      368 bytes                                                 ← no exported symbols
    $ grep -rc '#\[wasm_bindgen' src/                            → 1 match, inside a doc comment
    $ grep -c 'wasm-bindgen' wasm/Cargo.toml                     → 0
    ```

    All nine gates pass, including "release build for wasm". The build gate proves the crate **compiles**; nothing in
    CI loads the module, instantiates it, or calls `open_session` / `attempt_run` / `export_artifact_set` /
    `contract_version`. The Node harness that exists (`tests/host_verification/verify.mjs`) verifies a hand-written
    fixture artifact — it never touches the wasm build. So:

    - If the `#[wasm_bindgen]` layer never lands, **every gate stays green** and the crate still cannot be called.
    - The "bundle size" gate is in its `awaiting-packaging` state for the same reason: there is no artifact to measure
      (`measured_bytes: 0, budget_bytes: 0`), and it prints that honestly rather than pretending.
    - A 368-byte module passes the dependency-ban checks trivially — a tree with no exports links almost nothing, so
      "21 crates, no tokio/reqwest/libc" is a fact about a module that does nothing.

    This is the shape the crate's own gates were written to prevent elsewhere: a check that passes because it never
    had anything to check. The audit records it as "not implemented (4)" — but a not-implemented item inside a closed
    epic with 26/26 merged reads as done, and no gate would go red if it stayed that way forever.

  in_scope:
    - "A gate that loads the built wasm module and calls all four exported functions, failing on a missing export, an instantiation error, or a fifth function appearing unannounced — run in CI, in Node, against the release artifact rather than the source"
    - "A minimal assertion on each call: `contract_version` returns the three version numbers; `open_session` on a valid fixture returns a handle; `attempt_run` on the unlinked seam returns `kind: \"gate_unavailable\"` as `Ok`, not a throw; `export_artifact_set` before an attempt is a refusal"
    - "The bundle-size gate leaves `awaiting-packaging` only when that artifact exists, so the ceiling has a subject"
    - "The dependency-ban claim is stated against the shipped artifact, not a module with no exports"

  out_of_scope:
    - "The wasm-bindgen attribute layer itself, which is the packaging issue; this issue is the gate that fails until it lands"
    - "Publishing the npm package (#1346 covers train registration)"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "New: `wasm/ci/run-module-smoke.sh` (or equivalent) and its wiring in `run-wasm-gates.sh`; no library change"

  citations:
    - file: "wasm/ci/run-wasm-gates.sh"
      note: ":48 'release build for wasm' — the only gate that looks at the artifact, and it looks at compilation"
    - file: "wasm/ci/check-bundle-size.sh"
      note: ":74-83 the awaiting-packaging branch and its 'no exports yet' warning"
    - file: "wasm/src/interfaces/wasm_api.rs"
      note: ":17-19 the attribute layer deferred to the packaging issue"
    - file: "wasm/tests/host_verification/verify.mjs"
      note: "verifies a fixture artifact; never loads the module"

  verification:
    - "removing the wasm-bindgen attribute from one function fails the smoke gate, naming the missing export"
    - "a module that fails to instantiate fails the gate rather than skipping"
    - "the four calls behave as specified against today's unlinked seam"
---

# ISSUE-WASM-GATES-PASS-ON-A-MODULE-THAT-EXPORTS-NOTHING

## Measured

```
release cdylib            368 bytes
#[wasm_bindgen] in src/   0 (one mention, in a doc comment)
wasm-bindgen in Cargo     absent
run-wasm-gates            ✅ 9/9, including "release build for wasm"
anything that calls it    nothing
```

## Why this is the same defect class as everything else this quarter

A gate that passes because it never had a subject is indistinguishable from a gate that passes because the work is
right — until the day someone tries to call the module. Here the crate's own `check-bundle-size.sh` says it out loud
("no exports yet — not a shippable number"), and the size gate correctly refuses to enforce a ceiling; but the *build*
gate reports success, and the summary reads "wasm release build ✅".

The end state is one smoke gate that instantiates the artifact and calls four functions. It is cheap, it is the only
test that would notice if the crate were never callable, and it turns the size budget from a placeholder into a
measurement with an owner.
