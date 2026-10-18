# Batch 4 — Issue #1069

**Branch:** `issue/1069`

## Defect
`require_companion_step` was checked only at plan time over `&[PlannedStep]` —
no edges, no order, no outcomes. The name promises "the consequence is gated on
the companion"; the implementation delivered "include the companion somewhere".

## Fix — split the concept, keep both halves
| Check | Where | Question |
|---|---|---|
| presence (lint) | `evaluate_requirements` (plan) | does the plan contain a matching step? |
| execution prerequisite | `evaluate_companion_prerequisite` (dispatch) | has one **completed successfully**? |

- `CompanionStepObligationService::evaluate_prerequisite(completed, next)`.
- `SequencePolicyService::evaluate_companion_prerequisite` (default = none) so
  existing implementors/mocks are unaffected.
- `sequence_policy_verdict` (the single dispatch choke point) builds `completed`
  from `NodeStatus::Completed` nodes — a failed companion is excluded by
  construction — and returns `SequencePolicyVerdict::CompanionUnmet`.
- Both fill loops fail the node (`companion_prerequisite_unmet`) before its tool
  is called and release/skip dependents like any deterministic refusal.
- `promote` obligations are not enforced at dispatch: the human decision already
  happened at plan time, and re-denying would make approval unblockable.

## Tests
`tests/unit/sequence-policy/companion-prerequisite/` — 6 tests, including the
core regression: presence satisfies the lint, an empty completed set does not
satisfy the prerequisite.
