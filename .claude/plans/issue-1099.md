# Issue #1099 — the effect-scope oracle is disabled in all three entry points

**Branch:** `issue/1099-effect-oracle-wiring`

## State found
| element | state |
|---|---|
| `GitDiffEffectOracle` + `effect_scope_check` | present, tested |
| declared scope read from the approval's intent payload | present |
| `approval_repo_path` | `None` in **cli** and **actions**; never set in **mcp** (`..Default::default()`) |

So the check returns early for two independent reasons in every shipped entry point, and a run that
was never checked is byte-identical in output to a run verified clean.

## Fix
Set `approval_repo_path` to the repository root in all three roots. The oracle is already
`declared_scope`-gated, so wiring it cannot invent violations: no declared scope ⇒ early return.

One optimisation with the wiring: the per-run `git diff` baseline is now only taken when the graph
actually contains an approval-gated node. Wiring the path into every run without that would add two
git invocations per run that can never be used.

## Checkable, not just claimed
`rigorix doctor` gains a seventh row — `effect scope` — reporting whether the approval binding and
the repo path are both present. The wiring then has a surface that can be read, instead of a flag
buried in three builders.

## Tracker correction (#1072)
#1072 was closed `COMPLETED` with three parts; two landed and the third is this issue. The module doc
and ADR-011 now state which half landed, so the tracker and the docs agree.
