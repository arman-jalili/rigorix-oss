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

Legend: **CONFIRMED** = the claim holds, with the evidence named. **GAP** = the
claim does not hold; the document now says so and the row pins that admission.

| # | document | claim | verdict | evidence |
|---|---|---|---|---|
| W1 | `modules/capacity-ledger.md` | capacity is reserved **before** a step dispatches | CONFIRMED | the shared gate calls `self.capacity_verdict(` in `service_impl/gate.rs` |
| W2 | `modules/capacity-ledger.md` | the seam is armed from `.rigorix/capacity.toml` at composition time | CONFIRMED | `CapacitySetup::from_env` in CLI, Action **and** MCP host |
| W3 | `modules/precondition.md` | an operator check re-reads authoritative state before dispatch | CONFIRMED | the shared gate calls `self.precondition_verdict(` |
| W4 | `modules/sequence-policy.md` | the prefix gate runs at dispatch **and** the policy is consulted at plan time | CONFIRMED | gate calls `.sequence_policy_verdict(`; the orchestrator builder gets `with_sequence_policy(` |
| W5 | `modules/approval.md` | an approval is verified against the intent it approved, before dispatch | CONFIRMED | gate calls `verify_before_dispatch`; `ApprovalBindingSetup::from_env` in all three roots |
| W6 | `modules/approval.md` | a run's effects are diffed against the scope its approval declared | CONFIRMED | `approval_repo_path: Some` in all three roots + `self.effect_scope_check(` (was a GAP until #1099) |
| W7 | `modules/audit.md` | envelopes are tagged with the anchor regime and the verified head | CONFIRMED | `with_anchor(` in all three roots |
| W8 | `modules/sequence-policy.md` | with an anchor configured, cross-run history is signature-verified | CONFIRMED | `factory.rs` builds `AnchoredHistoryAdapter::new` |
| W9 | `modules/tool-system.md` | every command sink is one of the approved modules | CONFIRMED | `clippy.toml` denies `std::process::Command`; `apply_child_env` in the governed sink |
| W10 | `modules/hooks.md` | hooks run around tool calls | CONFIRMED | `hook_runner` is set in all three roots |
| W11 | `modules/failure-classification.md` | classification drives retry decisions | CONFIRMED | `factory_impl.rs` constructs `FailureClassifierServiceImpl` |
| W12 | `modules/enforcement.md` | permissions gate tool calls | CONFIRMED | `service_impl/tools.rs` calls `enforcer.check(` |
| **W13** | `modules/enforcement.md` | **budgets **RAII reservations** gate every action (README §1)** | **GAP** | **`ExecutionEnforcerImpl` has no caller outside `engine/src/enforcement/`; the enforcer is never constructed by an entry point** |
| **W14** | `modules/recovery-recipes.md` | **recovery recipes run when a step fails** | **GAP** | **`with_recovery_service` has no caller; `node.rs` consults `self.recovery_service`, which is always `None`** |

## What the two GAPs mean

Both are the same shape as the capacity ledger before `a861e2a8`: a complete
module with its own tests, referenced by the executor and by nothing else.

- **W13 is a product claim, not a doc nit.** The README says "Every execution runs
  inside operator-authored risk policy, permission and budget boundaries". Risk
  gating and permissions do run; contested capacity is enforced by the ledger; the
  per-execution **budget counter** does not run, because no composition root builds
  it. `README.md` §1 now says which of those is which.
- **W14 is narrower**: recovery recipes are an opt-in behaviour that has never been
  switchable, because nothing supplies the service.

Neither is fixed here. Wiring either one changes behaviour at dispatch — the
enforcer would start refusing calls that exceed a configured budget, and recipes
would start running on failure — so it is a decision with its own acceptance test,
not an audit edit. Tracked as **#1108**.
