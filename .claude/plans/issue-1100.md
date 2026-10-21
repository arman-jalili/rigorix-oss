# Issue #1100 — audit wiring claims against the call graph, and make the audit decay visibly

**Branch:** `issue-1100-wiring-claims`

## Deliverables
| Artifact | Purpose |
|---|---|
| `engine/.pi/architecture/wiring-claims.md` | the audit: 14 rows, verdict + evidence + how each is re-resolved |
| `engine/.pi/architecture/wiring-claims.tsv` | the same rows, machine-readable |
| `.pi/scripts/validate-wiring-claims.sh` | re-resolves every row each build; `--self-test` plants a regression |
| `.pi/guards.tsv` | registered **gating / run_here=yes**, so a stale claim fails the audit job |

## Two directions, both needed
- **CONFIRMED** row fails when its evidence disappears (wiring removed, doc still claims it).
- **GAP** row fails when (a) the document stops declaring the gap — the overclaim came back — or
  (b) callers appear, i.e. the gap was closed and the row must be promoted rather than left as a
  stale admission.

Direction (a) is not hypothetical: #1072 was closed `COMPLETED` with the wiring missing, and the
docs drifted back into claiming it (#1099).

## Findings
12 CONFIRMED (each seam / control with the construction site named per root).
**2 GAP**, both the same shape as the capacity ledger before `a861e2a8`:
- **W13** — `ExecutionEnforcerImpl` has no caller outside its own module: budgets and the autonomy
  presets are configured, validated, echoed and **never consulted at dispatch**. This contradicted a
  README headline claim, which is now corrected.
- **W14** — `with_recovery_service` has no caller: `node.rs` consults `self.recovery_service`, which
  is always `None`, so no recipe runs.

Wiring either changes dispatch behaviour (refusals, recovery steps), so both are recorded and filed
as **#1108** with an acceptance test requirement, not guessed at in an audit PR.

## Falsification
- self-test: planted CONFIRMED-without-evidence and GAP-without-admission → both caught; healthy
  manifest accepted.
- live: renamed `self.capacity_verdict(` in the gate → W1 failed with the file and needle named.
