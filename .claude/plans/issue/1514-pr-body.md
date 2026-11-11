# A document change did not reach the document guard

`.github/ci-affected.tsv` sent `docs/**` to the `docs` job, which runs three per-crate validators and **not** `.pi/scripts/validate-docs.sh`. That script is a gating guard (`run_here=yes`), and gating guards run only in `Audit` — so a pull request that rewrote seven specification documents reported success without running the validator whose subject is documentation. The same run had `wasm/.pi/architecture/decisions/**` matching no row at all.

This is the second time: `engine/.pi/**` had the same shape (#1498). So the fix is the rows **and** the class.

## The map

- `docs/**`, `*.md`, `**/*.md` → `docs,audit`.
- `**/.pi/**` → `audit` — every crate's guard tree, which `validate-script-drift.sh` reads: `actions/.pi/INDEX.md` previously went to `docs` and nowhere else.
- `.claude/**`, `.agents/**` → `audit` — prose a public guard reads. That is not hypothetical: this session's own PR body was caught there.
- Every row whose contents a `run_here=yes` guard reads gains `audit`.

**The cost is stated, not hidden:** `Audit` is now selected by more changes. A guard that does not run is not a gate, and the audit job takes ~4 minutes — cheaper than a green pipeline over unread documents.

## The class becomes checkable

`.pi/guard-subjects.tsv` (new) says what each of the sixteen `run_here=yes` guards reads and why. `.github/scripts/check_affected_map.sh` gains rule 5:

| rule | fails when |
|---|---|
| reachability | a guard's declared subject does not route to `audit` — naming the guard, the glob, a real path that matches it, and the subjects that path actually gets |
| staleness | a declared glob matches no tracked path |
| completeness | a `run_here=yes` guard declares nothing |

Seeds **S6** (a document whose routes all lose `audit`) and **S7** (an undeclared guard): 9/9 seeds pass. S5's expectation was updated from "a docs change selects only docs" to "docs and audit" — the old expectation was the bug.

## The wasm register: the scope is stated, and the findings are tracked

Running the engine guard against `wasm/.pi/architecture/decisions` reports four unresolved citations:

| file:line | finding |
|---|---|
| `ADR-010-artifact-format.md:115` | cites `ADR-016`, which that set does not contain |
| `ADR-010-artifact-format.md:102` | cites a path that resolves nowhere it could mean |
| `ADR-012-…:94` | `ADR-009 (error handling)` — names nothing in that ADR's subject |
| `ADR-013-…:111` | `ADR-005 (key custody)` — names nothing in that ADR's subject |

ADR numbering is **tree-local**, so resolving each one means deciding what the author meant in *that* tree — a judgement about another register's records, not a mechanical rewrite. So: the guard's header states the scope and the measurement, and the four findings are filed as #1520. I did not write the row into `guard-subjects.tsv`, because a table that claims a guard reads a directory it does not read is the defect this file exists to prevent.

## Acceptance

| # | Criterion | Command | Result |
|---|---|---|---|
| 1 | docs reach the guard | `ci_affected.sh --files docs/spec/execution-governance/01-terminology.md --list` | `docs` + `audit` |
| 2 | the guard bites on the tree | `validate-docs.sh` | ✅ green, and Audit is the job that ran it |
| 3 | wasm ADRs covered or stated | guard header + #1520 | stated, with the 4 findings measured |
| 4 | the map is checkable | `check_affected_map.sh` (+ `--self-test`) | ✅ total, 9/9 seeds |

Closes #1514.
