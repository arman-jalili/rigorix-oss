# Issue #1108 — wire the two modules no entry point constructed

**Branch:** `issue/1108-wire-budgets-and-recovery`

## Decision
**Wire both.** Both are real controls that were unreachable; deleting them would have removed the
capability instead of the gap. Recorded on the issue (edited, not just commented).

## W13 — budgets (#1108)
| Piece | Where |
|---|---|
| config | `ParallelExecutorConfig::enforcement: Option<EnforcementConfig>` |
| per-execution enforcer | `budget_enforcers: HashMap<dag_id, Arc<ExecutionEnforcerImpl>>` |
| evaluation | `service_impl/budget.rs` — **shared** by both dispatch paths |
| gates | `spawn_concurrent_node` (parallel loop) **and** `execute_tool` (single-node route) |
| roots | CLI (`[enforcement] preset`), Action (`standard`), MCP (`standard`) |

Both paths matter: the first version only gated `execute_tool`, which `execute_graph` never calls —
the test failed because the tool ran. That is #1074's shape; the fix was one shared function plus
the gate in the parallel loop.

## W14 — recovery (#1108)
`RecoverySetup::from_env()` (`RIGORIX_RECOVERY=1`) → `ParallelExecutionFactoryConfig::recovery` →
`with_recovery_service`. **Opt-in**, because recovery inserts an attempt into the failure path.
Honest limit recorded: `execute_step` is still simulated.

## Tests
`engine/tests/budget_enforcement_e2e.rs` — zero budget ⇒ `budget_exceeded` **and marker absent**;
within budget ⇒ step runs and marker exists (so the refusal is not blanket); recovery off by default.
Falsified by deleting the parallel-path gate: the marker appears.

## Docs/claims
`modules/enforcement.md`, `modules/recovery-recipes.md`, README §1 restored to what now runs;
rows W13/W14 promoted to CONFIRMED in `wiring-claims.tsv` with per-root/per-path evidence;
`rigorix doctor` gained `budget limits` and `recovery recipes` rows.
