# Issue #1094 — two paths in the demo README are missing `engine/`

**Branch:** `issue/1094` (single-issue batch; no grouping possible)

## Defect
`demo/capacity-ledger/README.md` tells the reader to look at
`.pi/architecture/modules/capacity-ledger.md` and `docs/runbook-capacity-ledger.md`.
Neither exists from the repo root — both live under `engine/`. They are backticked
prose, so `validate-docs.sh` (which checks relative **links**) never saw them.

## Fix (the issue's own prescription)
Turn both into relative markdown links, so the existing link guard covers them:

| line | was | now |
|---|---|---|
| §Guarantee ceiling | `` `.pi/architecture/modules/capacity-ledger.md` `` | `[…spec…](../../engine/.pi/architecture/modules/capacity-ledger.md)` |
| §Observing refusals | `` `docs/runbook-capacity-ledger.md` `` | `[…runbook…](../../engine/docs/runbook-capacity-ledger.md)` |

No guard extension: the same file also names `.rigorix/capacity.toml`, which is
legitimately absent (the demo creates it), so a blanket "backticked path must
exist" rule needs an allowlist immediately. That is a design question for later,
as the issue says.

## Falsification
Break one link and confirm `validate-docs.sh` fails, then restore — "the guard
covers it now" has to be observed, not assumed.
