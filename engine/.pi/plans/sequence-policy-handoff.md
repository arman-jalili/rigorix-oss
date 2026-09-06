# sequence-policy epic — session handoff (item 9+)

Pipeline PL-9787, tracking issue #837. **8/15 items merged.** Next:
item 9/15 `issue-execution-engine-r3` (#846, AC9), step: implement.

Full design brief (scoped, ready to implement) is on **issue #846 comment**:
https://github.com/arman-jalili/rigorix-oss/issues/846#issuecomment-5553422650
Plus earlier handoffs on #837 for items 1–8.

## Resume
```bash
cd engine
git checkout main && git pull          # green at 6ad5209c
git checkout -b feat/issue-execution-engine-r3
pipeline_status / pipeline_next_task    # item 9/15, implement
```
After `gh pr merge --delete-branch` always `git checkout -B main origin/main`.

## Merged so far (all local-CI 5/5 green, issues auto-closed)
| Item | MR | Issue |
|------|----|-------|
| 1 contract-freeze | #853 | #838 |
| 2 sequencerule | #854 | #839 |
| 3 sequencepolicyservice | #855 | #840 |
| 4 sequencepolicyerror | #856 | #841 |
| 5 steppredicate | #857 | #842 |
| 6 matcher | #858 | #843 |
| 7 orchestrator-r2 | #859 | #844 |
| 8 mcp-surface | #860 | #845 |

## Items 9–15
execution-engine-r3 (#846) — CRITICAL risk, brief on #846 · fail-closed (#847) ·
fail-open-absent (#848) · audit-r6 (#849) · permission-r5 (#850) · proofing
(#851) · architecture-readiness (#852).

## Workflow rules
- Validators from `engine/` only; gate = `validate-ci.sh` (5/5). The
  `validate-tests.sh` `--test integration` failure is a pre-existing main bug.
- `async_trait`/`tokio`/`serde_json`/`toml` are lib deps → behavior tests
  in-crate (`#[cfg(test)]`), scaffolds under `tests/unit/sequence-policy/` are
  contract-level only.
- `toml` isn't a dev-dep → TOML parse tests live in-crate in the domain file.
