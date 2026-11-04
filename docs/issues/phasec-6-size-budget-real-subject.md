---
guardian_issue:
  id: "ISSUE-PHASEC-6-SIZE-BUDGET-REAL-SUBJECT"
  title: "Phase C.6 — the size budget is re-recorded against the wired, default-built artifact, and the bundle-independence gate is re-checked against the engine in the module"
  epic: "portable-slice"
  component: "Wasm"
  module: "wasm-seam"
  status: planned
  priority: medium
  tracking_issue: 1386
  dependencies:
    - "Phase C.3 — the crate evaluates (must be merged)"
    - "Phase C.4 — the default build evaluates (must be merged)"
    - "Phase C.5 — goldens are CLI-produced (must be merged)"

  intent: |
    The budget's subject has changed twice already — from a raw cdylib, to a `wasm-bindgen` output
    with a debug section, to a stripped artifact. After C.4 the default build carries the engine, so
    the number the gate enforces is finally the number a host downloads.

    Two gates were written while the module linked nothing and must be re-checked now that it links
    the engine: the bundle-independence check (no forbidden crate in the module) and the domain
    isolation check (`std::time`/filesystem symbols absent from the gate path).

  in_scope:
    - "`wasm/ci/bundle-budget.txt` re-recorded: `measured_bytes` = the stripped, default-built module, `budget_bytes` = measured + 25 %, with the date, the commit and the command in the note"
    - "`check-bundle-independence.sh` proves its claim against a module that contains the engine: the seeded forbidden-crate case still fails, and the check reads the built module rather than the dependency declaration"
    - "`check-domain-isolation.sh` re-run: the engine's slice must not reintroduce a wall-clock or filesystem symbol into the gate path; a seed that adds one fails naming the symbol"
    - "The growth is recorded once: shipped-before (209 881 B) → shipped-after, with the delta attributed to the engine slice"

  out_of_scope:
    - "Trimming the module. If the re-recorded number matters, the levers are `data` (400 735 B of the emitted engine-linked module) and the `name` strip, not the dependencies — measure before optimising"
    - "The npm publication path"
    - "Changing the ceiling (1 500 000 B). It was decided in #1351 and the engine-linked artifact fits at 78 %; if a re-record exceeds it, that is a finding to file, not a ceiling to move"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "`wasm/ci/bundle-budget.txt`, `check-bundle-independence.sh`, `check-domain-isolation.sh`"
    interfaces:
      - "None"

  citations:
    - file: "wasm/ci/bundle-budget.txt"
      note: "the budget file and its provenance note"
    - file: "wasm/ci/check-bundle-independence.sh"
      note: "the forbidden-crate check written when the module linked nothing"
    - file: "wasm/ci/check-domain-isolation.sh"
      note: "the wall-clock/filesystem symbol check"
    - file: "docs/spike-portable-slice-2026-11.md"
      note: "§3 — 1 170 635 B shipped, 78.0 % of the 1 500 000 B ceiling, 341 785 B `name` section"

  verification:
    - "`bash wasm/ci/run-wasm-gates.sh` exits 0 and prints the measured default-build size; `bundle-budget.txt` `measured_bytes` equals it"
    - "Seed: skip the strip → the presence assertion fails and names the artifact (unchanged behaviour, re-proven on the new artifact)"
    - "Seed: add `reqwest` to a gate-path module behind the `engine` feature → `check-bundle-independence.sh` fails naming the crate"
    - "Seed: introduce `std::time::Instant::now()` on the gate path → `check-domain-isolation.sh` fails naming the symbol"
    - "The recorded delta between the pre-engine shipped module and the post-wiring one is stated in the budget note with both numbers"
---
**This issue is C.6 (#1386). Prerequisite: C.3 (#1383), C.4 (#1384) and C.5 (#1385).** Do not begin work before every
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

**C.3, C.4 and C.5 must be merged.** Re-recording a budget against an artifact that still links
nothing writes a number for a subject that does not exist — the defect `#1353` fixed once already.
