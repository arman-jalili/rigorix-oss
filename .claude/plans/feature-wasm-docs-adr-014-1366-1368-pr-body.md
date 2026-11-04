Three documents kept the pre-decision plan. The cost of that is not scholarly: a 31-day line for deleted work gets scheduled, and then the schedule is the reason the work gets built.

| | said | now |
|---|---|---|
| roadmap Phase 5 | "Engine Seam — FR-011 (Days 11–41, parallel)", 292-site inventory | Phase C: `EngineGate` on the real envelope, conformance `gate: "cli"`, **4 days** |
| roadmap Phase -1 | three assertions, unrecorded outcome | **branch D recorded** — report + ADR-014, and the one assertion it didn't reach named |
| roadmap total | 61 (51 after the spike) | **55 (45)**, with the arithmetic shown: 10+4+7+6+8+6+4+10 |
| runbook exit condition | "FR-011 (engine `Clock`/`IdSource` seam)" | unlinked seam + **Phase C** |
| runbook known limit | "Byte identity unproven until FR-011" | `c47b27f9…` at 853 B, pinned by `the_documented_pins_produce_the_golden_digest` |
| module doc | 0 × ADR-014; 19–22 "blocked until FR-011 (P2)" | cites ADR-014; restated against `PinnedInputs`; P2 marked superseded |
| runbook :5 | "eight gates" | **ten**, and the script asserts the sentence |

## #1367 — the permission whose precondition expired

`normalised_comparison=true` was permitted *"only while FR-011 is unimplemented"*. ADR-014 superseded FR-011, so that condition can never be true again, and the flag stayed available and unlabelled — the one shape the metric was built to forbid. An unpinned envelope is 884 B and a pinned one 877 B for the same decision; a comparator that normalises timestamps reports two different byte strings as equal.

Deleted, in every place it was granted or assumed:

- **module doc** criterion 22: no vector may set it, and one that does fails the gate naming it;
- **ADR-009**, where the interim was granted: struck through with the withdrawal and the reason;
- **`run_conformance_vector.rs`** contract comment: the field exists only so the assertion has somewhere to land;
- **roadmap** AC line: "refused" instead of "labelled interim";
- **`wasm/ci/run-conformance.sh`**: fails a vector carrying the flag.

The test `a_normalised_comparison_is_labelled` became `a_normalised_comparison_is_never_a_pass` — it asserted the interim *worked*, which is no longer a thing that exists.

## #1368 — a count nobody compared

`runbook.md:5` said eight; `grep -c '^gate "' wasm/ci/run-wasm-gates.sh` is ten. The script now compares its own declarations to the sentence **before** the gates run — a preflight, not a gate, because counting the gates must not change the count. It fails loudly if it cannot read the sentence at all: a check that silently matches nothing is narrower than the document it certifies.

## Seeds

```
SEED  runbook "ten gates" → "nine gates"
      ❌ runbook.md says 'nine gates' (9); this script declares 10
      restored → "runbook gate count: 10 (matches runbook.md)"

SEED  vec-0001.json gains "normalised_comparison": true
      ❌ vector(s) set normalised_comparison: true — that is a comparison of the thing
         normalised, not of the artifact (ADR-014, #1367):
         …/vectors/vec-0001.json
      restored → Conformance OK: 11 test(s) ran, and the seeded vector diverged as required

SEED  grep -c 'unproven until FR-011' runbook.md → 0
SEED  grep -c 'permitted only while' modules/portable-session.md → 0
SEED  grep -c 'ADR-014' modules/portable-session.md → 6
```

## One deviation, stated

`grep -rn 'normalised_comparison' wasm/` does **not** return "the criterion text only": the field survives on `ConformanceReport`, and two tests construct it. I kept it deliberately — it is a frozen contract type, and the field is where the "never a pass" assertion lands. Removing it would be a separate decision about that contract, not part of withdrawing a permission. What is gone is every claim that it may be `true`.

## Validation

```
wasm gates                                    10/10 (host verification labelled not-run)
cargo test -p rigorix-gate-wasm               181 passed, 0 failed
conformance harness                           11 tests, seeded vector diverged
docs / guards / canonical / tests / architecture / script-drift / shell-exit-status   all exit 0
cargo fmt --all --check                       clean
```

Closes #1366
Closes #1367
Closes #1368
