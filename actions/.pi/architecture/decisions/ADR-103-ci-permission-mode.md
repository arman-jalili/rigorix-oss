# Architecture Decision Record: ADR-103

<!--
Canonical Reference: .pi/architecture/decisions/ADR-103-ci-permission-mode.md
Blueprint Source: Rigorix design session (2026-06-20)
-->

## Title

ADR-103: CI environments default to `workspace_write` permission mode with PR-comment feedback

## Status

- [x] Accepted
- [ ] Deprecated
- [ ] Superseded

## Context

The engine's `PermissionEnforcer` supports three modes: `read_only`, `workspace_write`, and `dangerous_full_access`. In local interactive use, the default is to prompt for confirmation (`permission_mode: prompt`). But in CI (GitHub Actions):

- There is no human to confirm prompts
- The engine must write files to produce and validate code
- The workspace is ephemeral (cloned fresh each run)

The action must choose a safe, effective default permission mode for CI.

## Decision

**Default to `workspace_write` in CI environments.** The `CiDetector` checks for `GITHUB_ACTIONS=true` and sets the default permission mode to `workspace_write`. Confirmation prompts are replaced with:

- **PR comments**: structured execution summaries replace interactive prompts
- **Status checks**: green/red commit statuses replace terminal confirmations
- **Workflow annotations**: compiler errors appear as inline PR annotations

The mode can be overridden via the `permission-mode` input in `action.yml`.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| `read_only` in CI | Safest | Cannot write any files — template execution is impossible | Engine requires file writes for code generation |
| `dangerous_full_access` in CI | No restrictions | Allows arbitrary system modification — CI runner is a shared resource | Too permissive for CI |
| Prompt mode in CI | Same as local | Hangs forever waiting for human input | No human in CI |

## Consequences

**Positive:**
- CI runs complete without hanging on prompts
- Engine's workspace boundary check still applies — writes outside the workspace are denied
- PR comments provide equivalent feedback to interactive confirmation
- Override available via `permission-mode: dangerous_full_access` when needed

**Negative:**
- `workspace_write` allows the LLM to modify any file in the workspace
- Must trust the engine's risk gating and recovery loops to prevent destructive changes
- CI runner must have write access to the checkout directory

## Cross-References

- `engine/.pi/architecture/decisions/ADR-007-risk-gating-model.md` — Risk gating model
- `engine/.pi/architecture/modules/permission-enforcer.md` — Permission enforcer module
- `actions/.pi/architecture/modules/action-input.md` — CiDetector component spec

---

*Date: 2026-06-20*
*Session: rigorix-oss design session*
