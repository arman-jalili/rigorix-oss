---
guardian_issue:
  id: "ISSUE-PHASEC-2-CLI-ADOPTS-ENTRY-POINT"
  title: "Phase C.2 — the CLI calls the engine's gate entry point; its own composition of preconditions and sequence policy is deleted"
  epic: "portable-slice"
  component: "CLI"
  module: "gate-composition"
  status: planned
  priority: high
  tracking_issue: 1382
  dependencies:
    - "Phase C.1 — the engine gate entry point (must be merged)"

  intent: |
    C.1 adds the entry point; this issue makes the CLI its first caller, which is what turns the new
    function into the definition of a gate decision rather than an unused alternative to the CLI's own
    composition.

    Until the CLI calls it, the entry point is a second implementation of the ordering, and the wasm
    crate calling it would be agreeing with the engine while disagreeing with the product. The
    conformance claim in C.5 compares the crate against the *CLI*, so the CLI must be on the same
    function.

  in_scope:
    - "`cli/src/cli_boundary/orchestrator.rs` calls the entry point instead of building `sequence_policy` and wiring `PreconditionDispatchGate` itself — `:338`, `:418`, `:700`, `:718-719`"
    - "The CLI's existing tests pass **unmodified**; a test that needs editing means the entry point changed behaviour, which this issue does not permit"
    - "The type-visibility audit: the entry point is public in `rigorix-engine`, so it appears in the engine's public API surface — the CLI is a published crate and a consumer, not a special case"

  out_of_scope:
    - "Changing any decision the CLI makes, including which findings are produced and in what order"
    - "Removing the CLI's ability to configure ports — it supplies them to the entry point exactly as it supplies them today"
    - "The crate, conformance, the demo"

  affected_layers:
    domain:
      - "None"
    application:
      - "`cli/src/cli_boundary/orchestrator.rs` — composition replaced by one call"
    infrastructure:
      - "None"
    interfaces:
      - "None — no CLI flag, output or exit code changes"

  citations:
    - file: "cli/src/cli_boundary/orchestrator.rs"
      note: ":338 and :700 `let sequence_policy = …`; :418 `sequence_policy: sequence_policy.clone()`; :718-719 `with_sequence_policy`"
    - file: "cli/src/cli_boundary/dispatch.rs"
      note: "the dispatch path these builders feed; unchanged"

  verification:
    - "`cargo test -p rigorix-cli` passes with **zero edits to any test file**: `git diff --stat cli/tests cli/src/cli_boundary/tests.rs` is empty"
    - "`grep -c 'sequence_policy' cli/src/cli_boundary/orchestrator.rs` returns 0"
    - "`git diff cli/src/cli_boundary/orchestrator.rs | grep -c '^-'` shows deletions only in the composition sites, not in behaviour"
    - "Seed: revert one call site to the old composition → the determinism test in C.1 fails, proving both callers were on one function and not two"
    - "A CLI run against the pilot template produces byte-identical output to the same run before this change (`sha256` recorded and compared)"
---
**This issue is C.2 (#1382). Prerequisite: C.1 (#1381).** Do not begin work before every
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

**Phase C.1 must be merged.** Do not inline a copy of the entry point's body here, and do not keep the
old composition behind a flag "just in case": two live compositions is the defect C.1 exists to
remove. If the entry point is missing something the CLI needs, file the delta against C.1.
