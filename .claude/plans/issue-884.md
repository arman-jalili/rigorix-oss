# Issue #884 — F-20260921-02 (GAP-A-28): Operator-controlled step requirements (ADR-015)

Branch: `issue/884`
Module: `sequence-policy`
Spec: `engine/.pi/architecture/decisions/ADR-015-operator-step-requirements.md`
        `engine/.pi/architecture/modules/sequence-policy.md#r9`

## Problem (verified)
Plan-declared `require_identity` and canonical step parameters (`/beneficiary`,
`/effect_key`) are agent-controllable for `rigorix_execute` plans. An operator
cannot state "any step matching P MUST be attested / MUST carry Q". ADR-015 adds
`[[requirements]]` — obligations evaluated at plan time for every plan.

## Implementation order
1. Domain `StepRequirement` / `RequirementAction` / `RequirementFinding`
   (`engine/src/sequence_policy/domain/requirement.rs`, new).
2. `SequencePolicyConfig.requirements` + `SafetyCaps` count/pointer caps +
   fail-closed `validate()`.
3. `SequencePolicyService::evaluate_requirements` (trait + impl).
4. Events `RequirementUnmet` / `RequirementPromoted` + exhaustive match sites.
5. Audit `requirement_findings[]` (`AuditEnvelope` + factory).
6. Orchestrator plan-time gate `apply_plan_time_requirements` (adjacent to the
   HIGH-impact `apply_plan_time_sequence_policy`, signature unchanged);
   `OrchestratorError::RequirementUnmet`; `PlanOnlyOutput.requirement_findings`.
7. MCP `rigorix_validate_plan` surfaces requirement findings; error taxonomy.
8. Tests: unit (domain, config, repository, service) + integration
   (orchestrator + determinism property).
9. Docs: CHANGELOG, ADR-015 → Accepted.

## Semantics (ADR-015, frozen)
- `require_identity` unmet (absent or `Unverified`) → refuse (`IdentityRequired`),
  **never promotable**.
- `require_params` missing/null pointer → `deny` (`RequirementUnmet`) or
  `promote` (`requires_approval = true` on the matched step).
- Findings are additive evidence; pointer **names** only (values never captured).
- Requirements are **additional** to plan-declared `require_identity`.
- Malformed requirements fail closed; absent = status quo.

## Acceptance criteria (R9)
AC 18 identity refuses unauthenticated even when step omits flag; never promotable.
AC 19 require_params deny/promote.
AC 20 applies to `rigorix_execute` plans; validate_plan surfaces findings.
AC 21 envelope `requirement_findings[]`; values redacted.
AC 22 malformed fail-closed; absent = status quo; SafetyCaps bound count + pointers.
