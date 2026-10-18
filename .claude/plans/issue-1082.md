# Issue #1082 — failure-class catalogue + one guard per class

**Branch:** `issue/1082`

## Catalogue
`engine/.pi/architecture/failure-class-ledger.md` — 8 classes, each a property of
the codebase, with guard, status and **falsification**.

## Built here (C2, C3, C5, C6) + carried from #1083 (C1) + C8 script
| Class | Change | Falsified by |
|---|---|---|
| C2 | `git_read_tool` executes **argv**, not `sh -c` (a caller `path` was interpolated); write/exec flag refusal; `guard_suite::c2_*` pins the shell-sink set | `check-red-green.sh origin/main` → RED: `git diff --output=…` **was** accepted before the fix |
| C3 | hooks call `apply_child_env` — the hook child inherited `RIGORIX_HMAC_KEY` | removing the call → test fails, `denied: true` |
| C5 | `guard_suite::c5_*` over a manifest of decision-bearing fields | a declared-but-never-consulted field fails |
| C6 | `guard_suite::c6_*` — no `*from_env` returns a bare `Option` | `pub fn bad_setup_from_env() -> Option<u8>` fails |
| C8 | `.pi/scripts/check-red-green.sh` | demonstrated: probe RED on `origin/main`, green on branch |

## Not built (honest)
**C7** (reachability) needs the call graph, not grep — specified in the ledger with
the exact query and why a grep answers it wrongly in both directions. **C4** is
partly built (the two label/provenance fixes) and stays type-level where possible.
