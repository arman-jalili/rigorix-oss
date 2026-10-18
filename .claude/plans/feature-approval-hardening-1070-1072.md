# Batch 2 — Issues #1070 + #1072

**Branch:** `feature/approval-hardening-1070-1072`
**Impact:** `gitnexus_detect_changes` = **CRITICAL** (30 affected symbols) because
`ApprovalBindingSetup::from_env` is called from all three composition roots
(`build_orchestrator_with_budget`, `build_action_orchestrator`, `build_mcp_state`).
That is exactly the intended blast radius: the signature change makes the silent
downgrade impossible, and the compiler found every caller.

## #1070 — requested guarantee must not vanish
- `ApprovalBindingSetup::from_env -> Result<Option<Self>, ApprovalBindingError>`:
  - `Ok(None)` = not requested (quiet).
  - `Err(MissingKey)` = `RIGORIX_APPROVAL_BINDING=1` with no/empty `RIGORIX_HMAC_KEY`.
  - `Err(StoreUnavailable)` = store cannot open (replaces an `expect` panic).
- `from_env_values` keeps the decision testable without `set_var` (edition 2024).
- CLI / Action / MCP propagate: no process starts "binding" while approving unbound.
- Panic baseline 27 → 26 (the `expect` is gone).

## #1072 — say what the oracle actually covers
- Snapshot now includes `git diff --cached`: a staged edit is no longer invisible.
- Docs (oracle, ADR-011 §5, module, runbook) state: **off by default** (all entry
  points pass `None`), per-loop baseline (union under concurrency, no per-step
  attribution), paths-not-content, and a dirty working tree is reported as the
  run's effect.
- Decision: **keep it off and say so** (the issue's option 4) rather than wire a
  per-loop oracle whose false positives on a dirty tree would be worse than none.

## Tests
- 3 binding-env tests (quiet / fail / armed).
- `staged_changes_are_part_of_the_snapshot`.
- Full CI 5/0, workspace clippy `-D warnings`, architecture/canonical/security/docs.
