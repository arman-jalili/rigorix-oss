# Issue Implementation Groups

**Generated:** 2026-02-13
**Total Issues:** 4 (open, fetched by `.pi/scripts/fetch-issues.sh`)
**Branch:** `feature/sink-register-1478-1481`

---

## Grouping Strategy

These four issues carry **no labels**; priority and component are declared in the
`guardian_issue:` YAML block of each body. `categorize-issues.sh` reads labels, so its
"4 individual issues" output is a false negative here — this file is the classification.

Grouped by:
1. **Component** — `tools/application` (2), `audit`+`orchestrator` (1), `.pi/architecture` (1)
2. **Priority** — `high` ×3, `medium` ×1 (from `guardian_issue.priority`)
3. **Dependency** — #1481 (`SINK-3`) records the mitigations #1478/#1479/#1480 create,
   and its acceptance #5 requires "the register matches reality", so it lands last
4. **Shape** — all four are the same class: *a control is present and is not the control
   the sentence claims* (a filtering recorded as isolation, a check separated from its
   mutation, a result computed and dropped). One branch, four commits, one MR.

---

### Group: Sink & evidence mechanisms (`SINK-1/2/3`, `EVID-1`)

**Branch:** `feature/sink-register-1478-1481`
**Component:** `engine/src/tools/application`, `engine/src/audit/application`,
`engine/src/orchestrator/application`, `engine/.pi/architecture`
**Issues:** 4

| Order | Issue | Priority | Component | Files |
|---|---|---|---|---|
| 1 | #1478 `SINK-1` | high | run-command | `shell_policy.rs`, `run_command_tool.rs`, `docs/deployment.md` |
| 2 | #1479 `SINK-2` | medium | file-write | `file_write_tool.rs`, new `tools/infrastructure/fs_at.rs` |
| 3 | #1480 `EVID-1` | high | evidence path | `audit/application/*`, `orchestrator/application/*` |
| 4 | #1481 `SINK-3` | high | the mechanism | `sink-register.tsv`, `validate-sink-register.sh`, `.pi/guards.tsv` |

**Plan:** `.claude/plans/feature-sink-register-1478-1481.md`
**MR body:** `.claude/plans/feature-sink-register-1478-1481-pr-body.md`

**Validation per issue**
| Issue | Validators |
|---|---|
| #1478 | security, docs, tests |
| #1479 | security, tests |
| #1480 | tests, security, canonical |
| #1481 | security, architecture, ci, canonical |

**Exposure, stated once:** #1478 and #1479 are *latent* — no shipped binary constructs
`RunCommandTool` or `FileWriteTool` (every site is `#[cfg(test)]`; the MCP host dispatches
catalog methods only). They become reachable the moment a library consumer wires the tools
module, which is why they are fixed before that rather than after. #1480 is live: it is on
the orchestrator's run path.

---

### Individual Issues (No Grouping)

None. All four issues share one mechanism and one branch.
