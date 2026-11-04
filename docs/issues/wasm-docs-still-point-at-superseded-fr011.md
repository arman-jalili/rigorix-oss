---
guardian_issue:
  id: "ISSUE-WASM-DOCS-STILL-POINT-AT-SUPERSEDED-FR011"
  title: "Three wasm documents still send a reader to the Clock/IdSource seam ADR-014 removed, and one of them budgets 31 days for it"
  epic: "portable-slice"
  component: "Docs"
  module: "wasm-seam"
  status: planned
  priority: medium
  tracking_issue: 1366
  dependencies:
    - "ADR-014 (the decision these documents predate)"
    - "#1355 (proved byte identity for one operation)"

  intent: |
    ADR-014 decided two things: `portable`'s exclusions are a manifest, and determinism on the gate
    path is one operation plus `PinnedInputs` — **not** a crate-wide `Clock`/`IdSource` abstraction.
    #1355 then proved the point: one threaded site, and `c47b27f9…` identical from the CLI and the
    crate, pinned by `the_documented_pins_produce_the_golden_digest`.

    Three documents were not told:

    **`implementation-roadmap.md:358`** — `## Phase 5: Engine Seam — FR-011 (Days 11–41, parallel
    cross-crate track)`, whose goal paragraph still reads "an additive `Clock` + `IdSource` seam
    covering the gate path, defaulting to system time, reachable from the CLI behind a non-default
    feature". One deliverable row is annotated as superseded; the phase's *title, day range and
    goal* are not. The schedule totals 61 days (51 after the spike) and 31 of them belong to work
    the ADR removed.

    **`runbook.md`** — §3's exit condition for `gate_unavailable` is "FR-011 (engine `Clock`/`IdSource`
    seam)", the known-limits table says "FR-007 blocked on FR-011", and line 148 says "Byte identity
    unproven until FR-011 — the CLI cannot yet be given the same pinned clock/ids". The last is
    false: the CLI takes `--pinned-clock`/`--pinned-ids`, both runtimes produce one digest, and a
    test pins it.

    **`modules/portable-session.md`** — **zero** mentions of ADR-014, the canonical decision for the
    module it describes. Its note at :502 says "19, 20 and 21 are blocked until P2 (FR-011) lands",
    and criterion 22's permission hinges on "while FR-011 is unimplemented" (see the companion issue
    about that clause).

    The consequence is not scholarly. The next person to unblock FR-007 reads the runbook, finds an
    exit condition naming a seam, and — if the roadmap is opened next — 31 days of budget for it.
    ADR-014 exists precisely so that nobody rebuilds that.

  in_scope:
    - "Roadmap Phase 5: title, goal and day range restated as what remains — Phase C's `EngineGate` wiring and the conformance flip — and the total recomputed from the phases that still exist"
    - "Roadmap Phase -1: its outcome recorded (branch D, `docs/spike-portable-slice-2026-11.md`) so the spike reads as done rather than planned"
    - "Runbook §3 and the known-limits rows: the exit condition for `gate_unavailable` is the unlinked seam and Phase C, not FR-011; byte identity is recorded as proven for one operation with the witness and the test that pins it"
    - "The module doc cites ADR-014 by path and restates criteria 19–21 against `PinnedInputs` and `with_pinned_inputs`"

  out_of_scope:
    - "Deleting the `Clock`/`IdSource` ports from `portable-session`'s port table. They remain for the session's own reasons where a second consumer exists; ADR-014 removed them from *this* seam"
    - "The report, which already records the branch-D outcome and the one-site finding"
    - "Any engine or crate code"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "None"
    interfaces:
      - "None"

  citations:
    - file: "wasm/.pi/architecture/implementation-roadmap.md"
      note: ":358 the Phase 5 heading and goal; :361 the seam deliverable; :368 the superseded annotation; :498 the 61-day total"
    - file: "wasm/.pi/architecture/runbook.md"
      note: ":85 'once FR-011 lands'; :102-104 the exit condition; :144 and :148 the known-limits rows"
    - file: "wasm/.pi/architecture/modules/portable-session.md"
      note: ":48 the ADR table (ADR-014 absent); :502 the blocked note; :529 criterion 22"
    - file: "wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md"
      note: "§2 — determinism witnessed by one operation and a carrier, not by a clock abstraction"

  verification:
    - "`grep -rn 'FR-011' wasm/.pi/architecture/` returns only historical references, each adjacent to an ADR-014 pointer or the spike report — no document names FR-011 as the current exit condition"
    - "The module doc cites ADR-014: `grep -c 'ADR-014' wasm/.pi/architecture/modules/portable-session.md` is ≥ 1"
    - "The roadmap's total equals the sum of the phases it still lists, and the Phase 5 heading no longer carries a day range for the removed seam"
    - "The runbook's known-limits row for byte identity states the witness `c47b27f9…` and names the test that pins it, not 'unproven'"
    - "Seeded sentence check: `grep -c 'unproven until FR-011' wasm/.pi/architecture/runbook.md` is 0"
---

# The decision landed; three documents kept the old plan

| document | still says | now true |
|---|---|---|
| roadmap :358 | Phase 5 = FR-011, Days 11–41 | one site (#1355), and no seam (ADR-014) |
| runbook :102 | exit condition = FR-011 seam | unlinked seam + Phase C |
| runbook :148 | byte identity unproven until FR-011 | `c47b27f9…` from both runtimes, golden-pinned |
| module doc | 0 × ADR-014; 19–21 blocked on FR-011 | restate against `PinnedInputs` |

A planning document that keeps a 31-day line for deleted work spends real money: someone will
schedule it, and the schedule will be the reason the seam gets built after all.
