# Wiring-claims audit (#1100)

This file exists because the same failure repeated: **code that exists, compiles,
passes its own tests, and is never reached.**

| instance | how it was found |
|---|---|
| capacity ledger entirely unwired | the module doc had no `EnforcementSeam` component, so no wiring was ever planned |
| effect-scope oracle disabled in all three roots | reading the module doc against the entry points; #1072 was then closed `COMPLETED` anyway (#1099) |
| `Anchored` label from configuration rather than a head | #1071 |
| a step's approval resolved, the declared scope never compared | #1072 → #1099 |

None of those is a compile error, a test failure or a type mismatch. Each is a
**claim of wiring that was true of the code in isolation and false of the system.**
Grep finds the symbol; only reachability answers the question.

## Method

Every row below was checked by resolving the claim against the code, not against
the prose that repeats it:

| method | finds | misses |
|---|---|---|
| grep for a symbol | that the code exists | that nothing calls it |
| the test suite | the tested path | an untested sibling path |
| reading the doc | what the doc says | whether it is true |
| **reachability per claim** | whether anything reaches it | — |

The audit used the call graph (`gitnexus_impact`, upstream) for the two **GAP** rows,
where the question was "does *anything* reach this?", and file-level construction
sites for the **CONFIRMED** rows, where the question was "does *each entry point*
construct it?" — three roots have to be checked by name, because two out of three is
the failure mode (#1099 had exactly that shape).

## Adding the next row

1. Resolve the claim against the code first: grep finds the symbol, only
   reachability answers "does each entry point construct it?". Check the roots
   by name — two out of three is the failure mode this manifest exists for.
2. Choose a check that fails when the wiring disappears: a `contains` on the
   call site (path + identifier), not on the module's own definition; a
   `no-callers` when the point is that nothing reaches it.
3. Add the row to **both** files — `wiring-claims.tsv` (the check) and the table
   below (the reasoning) — with the same id.
4. Run `.pi/scripts/validate-wiring-claims.sh`; a row that cannot fail is not a
   check.

## The check that decays visibly

A one-off audit is true for one commit. `wiring-claims.tsv` is the machine-readable
form of this table, and `.pi/scripts/validate-wiring-claims.sh` re-resolves every
row on each build:

- a **CONFIRMED** row fails when its evidence disappears (the wiring was removed);
- a **GAP** row fails when (a) the gap is no longer declared in its document — i.e.
  someone quietly restored the overclaim — or (b) callers appear, meaning the gap
  was closed and the row should be promoted to CONFIRMED.

That second direction is the one that matters for this series: the docs drifted
back into claiming wiring *after* it was known to be missing, twice.

## Audit

Legend: **CONFIRMED** = the claim holds, with the evidence named.
**CONFIRMED (conditional)** = the claim holds only under the condition stated in
the row; the evidence for both branches is named. **GAP** = the claim does not
hold; the document now says so and the row pins that admission.

### Conditional rows

Some wiring is true per backend, per declaration, or per opt-in flag. Collapsing
that into one unconditional CONFIRMED is how W17 came to say "every root sweeps"
when an L2 root deliberately declines. A conditional row must:

1. name the condition in the verdict (`CONFIRMED (conditional — <condition>)`);
2. state both branches in the claim, not just the one that is true today;
3. carry a check for the selector (the capability, config call, or flag), not
   only for the call site. The guard checks presence, not prose — the claim text
   is what tells the next reader which branch they are on.

| # | document | claim | verdict | evidence |
|---|---|---|---|---|
| W1 | `modules/capacity-ledger.md` | capacity is reserved **before** a step dispatches | CONFIRMED | the shared gate calls `self.capacity_verdict(` in `service_impl/gate.rs` |
| W2 | `modules/capacity-ledger.md` | the seam is armed from `.rigorix/capacity.toml` at composition time | CONFIRMED | `CapacitySetup::from_env` in CLI, Action **and** MCP host |
| W3 | `modules/precondition.md` | an operator check re-reads authoritative state before dispatch | CONFIRMED | the shared gate calls `self.precondition_verdict(` |
| W4 | `modules/sequence-policy.md` | the prefix gate runs at dispatch **and** the policy is consulted at plan time | CONFIRMED | gate calls `.sequence_policy_verdict(`; the orchestrator builder gets `with_sequence_policy(` |
| W5 | `modules/approval.md` | an approval is verified against the intent it approved, before dispatch | CONFIRMED | gate calls `verify_before_dispatch`; `ApprovalBindingSetup::from_env` in all three roots |
| W6 | `modules/approval.md` | when the approved intent declares a scope, the run's effects are diffed against it; plan creation declares it for file actions | CONFIRMED (conditional — declared scope) | `approval_repo_path: Some` in all three roots + `self.effect_scope_check(`; `declared_scope` is written by `planning/application/pipeline_impl.rs` for file actions (was a GAP until #1099) |
| W7 | `modules/audit.md` | envelopes are tagged with the anchor regime and the verified head | CONFIRMED | `with_anchor(` in all three roots |
| W8 | `modules/sequence-policy.md` | with an anchor configured, cross-run history is signature-verified | CONFIRMED | `factory.rs` builds `AnchoredHistoryAdapter::new` |
| W9 | `modules/tool-system.md` | every command sink is one of the approved modules | CONFIRMED | `clippy.toml` denies `std::process::Command`; `apply_child_env` in the governed sink |
| W10 | `modules/hooks.md` | hooks run around tool calls | CONFIRMED | `hook_runner` is set in all three roots |
| W11 | `modules/failure-classification.md` | classification drives retry decisions | CONFIRMED | `factory_impl.rs` constructs `FailureClassifierServiceImpl` |
| W12 | `modules/enforcement.md` | permissions gate tool calls | CONFIRMED | `service_impl/tools.rs` calls `enforcer.check(` |
| W13 | `modules/enforcement.md` | budgets and per-tool `max_calls` are evaluated before a tool call runs | CONFIRMED (#1108) | `service_impl/budget.rs` calls `evaluate_tool_call(`; both dispatch paths call it; `enforcement: Some(` in all three roots |
| W14 | `modules/recovery-recipes.md` | recovery recipes can be attached when a step fails | CONFIRMED (conditional — `RIGORIX_RECOVERY`) | `RecoverySetup` + `with_recovery_service(` in the factory; `RecoverySetup::from_env(` in all three roots |
| W15 | `modules/effect-commitment.md` | every root calls `from_env`; a present `.rigorix/effect-commitment.toml` arms the seam, an absent one leaves it off | CONFIRMED (conditional — declaration present) | `EffectCommitmentSetup::from_env` in CLI, Action **and** MCP host; absent ⇒ `Ok(None)` |
| W16 | `modules/effect-commitment.md` | when the seam is armed, the intent is recorded before dispatch and settled after | CONFIRMED (conditional — seam armed) | `factory_impl.rs` calls `with_effect_commitment_seam(`; `dispatch.rs` calls `seam.before_dispatch(` |
| W17 | `modules/effect-commitment.md` | every root calls `reconcile`; the ledger capability decides — L1 sweeps, L2 declines and the service owns it | CONFIRMED (conditional — L1 sweeps, L2 declines) | `.reconcile()` in CLI, Action **and** MCP host; `can_sweep_dispatched()` is `true` in `in_memory.rs`, `false` in `http.rs`, and `reconcile.rs` gates on it |

## The two rows that were GAPs (closed by #1108)

Both had the shape the capacity ledger had before `a861e2a8`: a complete module
with its own tests, referenced by the executor and constructed by nothing.

- **W13** — `ExecutionEnforcerImpl` had no caller outside `engine/src/enforcement/`.
  Budgets, per-tool `max_calls` and the presets were configured, validated, echoed
  and never consulted. Now every entry point attaches an `EnforcementConfig`, and
  **both** dispatch paths evaluate each call before the tool runs — the parallel
  loop and the single-node route — through one shared `service_impl::budget`
  function, because a check added to one path only is the #1074 defect.
- **W14** — `with_recovery_service` had no caller. Now `RecoverySetup::from_env()`
  arms it (`RIGORIX_RECOVERY=1`) and the factory attaches it. Opt-in, because
  recovery inserts an attempt into the failure path and changes how a failure is
  retried; wiring makes it reachable and switchable without silently changing
  failure handling for everyone.

The lesson the audit recorded still holds: both were invisible to grep, to the
test suite and to review, and neither was a compile error. They became visible
only when the claim was resolved against what constructs what.
