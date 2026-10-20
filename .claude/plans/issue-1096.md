# Issue #1096 — `.pi/context/domain-workflow.md`: two dead paths, one truncated

**Branch:** `issue/1096` (single-issue batch)

## Defect (as filed)
| row | referenced | actual |
|---|---|---|
| 75 | `.pi/architecture/decisions/ADR-001.md` | `engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md` |
| 76 | `.pi/architecture/diagrams/system-context.md` | `engine/.pi/architecture/diagrams/system-context.md` |

Prefix missing on both; the ADR row also truncates the filename, so fixing the
prefix alone still leaves a dead path.

## Scope note found while fixing
The file is **mirrored identically** into `actions/`, `cli/`, `engine/` and `mcp/`.
The same two rows are wrong in all four copies, so all four are fixed — leaving
three knowingly-broken copies is not a smaller change, it is the same defect.

Fix: root-relative paths with the full ADR name, plus a one-line convention note
above the table ("Paths are relative to the repository root") so the next reader
does not have to guess the base directory.

## Method correction (the issue's main point)
My #1094 claim — "I scanned and it was limited to those two lines" — was wrong,
and the reason matters: the scan output was truncated and I generalized from the
visible part. The real numbers, with the three-resolution method the issue
describes (doc-dir, repo root, `engine/`):

- **1233** backticked paths resolve under none of the three;
- **370** of those look root-relative (start with `.pi/`, `docs/`, `engine/`, `actions/`, `cli/`, `mcp/`, `server/`, `demo/`, `schemas/`).

Neither number is a to-do list: the repo uses **four** conventions (module-relative,
repo-root, crate-root, doc-dir-relative), which is exactly why no guard exists.
That is the separate design question the issue records, and this PR does not
pretend to settle it.

## Out of scope (recorded, not silently corrected)
The four scaffold references (`.opencode/context.md`, `.github/copilot-instructions.md`,
`.github/copilot/settings.json`, `.github/workflows/01-planning-workflow.md`) point at a
layout that was never instantiated. Deleting them or making the scaffold real is a
decision, as the issue says.
