# Issue Implementation Groups

**Generated:** Tue Nov  3 19:36:23 CET 2026
**Total Issues:** 1

---

## Grouping Strategy

Issues are grouped by:
1. **Component** — Same module/files affected
2. **Priority** — Critical > High > Medium > Low
3. **Dependency** — Blocking relationships

---


### Individual Issues (No Grouping)

Each requires separate branch: `issue/{number}`

- #1351: Spike: can rigorix-engine's gate slice compile to wasm32, produce a byte-identical envelope, and fit a demo-viable bundle → branch: `issue/1351`

---

## Coordinator note

| Question | Answer |
|---|---|
| Component batch (2–5, same module)? | No — one open issue |
| Priority batch (all critical/high)? | No — one issue |
| Related batch (dependencies linked)? | No — its dependencies (#1346 train, #1349 export layer) are closed |
| **Single** | **Yes** → `issue/1351` |

Priority is **high** (from the issue's `guardian_issue.priority`), even though no label is set.

### Shape of the PR (decided here, because it constrains the plan)

#1351 is a **spike**. §5: *"Merge nothing but the report and the inventory."* §6 deliverables:
`docs/spike-portable-slice-2026-11.md` and `engine/.pi/portable-inventory.tsv`. The `portable`
feature, the `spike-m2` harness and the CLI pinned-input flags are measurement apparatus — the
issue asks for numbers, an inventory and one of four branch verdicts, not a half-threaded seam.
So: measure on the branch, record commands + outputs, and merge only what §6 names.
