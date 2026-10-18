# Batch 6 — Issue #1066

**Branch:** `issue/1066`

## Defect
The `BudgetScope` doc comments instructed operators to set `shared_contested`
"for a limit the capacity ledger enforces" — the exact scope that `validate()`
always refuses (#1061). Correct error, misleading path to it.

## Fix (doc-only + one test)
- Enum doc: a contested limit is **not** a budget; declare it in
  `.rigorix/capacity.toml`.
- `SharedContested` variant: retained so existing configs deserialize, **rejected
  by validation**, do not set it.
- `ResourceBudget` + `scope` field docs: same instruction, same wording.
- New test pins that the refusal message names `.rigorix/capacity.toml` and
  "remove this budget", so message and docs cannot drift apart again.
