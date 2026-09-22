## Summary

Implements **#884 / F-20260921-02 (GAP-A-28)** — operator-controlled step requirements (ADR-015).

Plan-declared `require_identity` and canonical step parameters (`/beneficiary`,
`/effect_key`) are agent-controllable for composed runs: an agent running
`rigorix_execute` can omit them, escaping both the value-identity rule (R8) and
effect-keyed history. This adds an **operator-owned** obligation over matched
steps that the plan cannot remove.

```toml
[[requirements]]
id = "payout-guard"
name = "Payout commands must be attested and carry canonical effect data"
match = { tool = "run_command", params = [{ pointer = "/command", kind = "glob", value = "*execute_payout.sh*" }] }
require_identity = true
require_params = ["/beneficiary", "/effect_key"]
action = "deny"   # deny (default) | promote
```

## What changed

- **Domain (`sequence_policy/domain/requirement.rs`, new)** — `StepRequirement`
  (reuses the frozen `StepPredicate` matcher), `RequirementAction`
  (`deny` default / `promote`), `RequirementFinding` (redacted summary).
- **Config** — `SequencePolicyConfig.requirements` (serde default, skipped when
  empty → pre-R9 configs serialize unchanged); fail-closed `validate()`
  (unique ids, ≥1 obligation, pointers start `/`, no `equals_step` in a
  single-step match); `SafetyCaps.max_requirements_per_file` +
  `max_required_params_per_requirement`.
- **Service** — `SequencePolicyService::evaluate_requirements` (deterministic:
  config order, then step order).
- **Orchestrator** — `apply_plan_time_requirements` runs after the
  sequence-policy gate over the enforced list, for every plan:
  - `require_identity` unmet → `IdentityRequired` — **never promotable**.
  - `require_params` unmet + `deny` → `RequirementUnmet` (plan refused); +
    `promote` → `requires_approval = true` on the matched step.
- **Evidence** — `ExecutionEvent::RequirementUnmet` / `RequirementPromoted`;
  additive envelope `requirement_findings[]` (**pointer names only** — values
  redacted per SpanPrivacy); `rigorix_validate_plan` surfaces
  `requirement_findings`.
- **Docs** — ADR-015 → **Accepted**; CHANGELOG; workstreams note.

The HIGH-impact `apply_plan_time_sequence_policy` was left unchanged; the new
gate is adjacent and composes with it.

## Acceptance criteria (R9)

| AC | Status |
|----|--------|
| 18 — `require_identity` refuses even when the step omits the flag; never promotable | ✅ unit + orchestrator |
| 19 — `require_params` deny when missing / allow when present; `promote` sets approval | ✅ unit + orchestrator |
| 20 — applies to `rigorix_execute` composed plans; `validate_plan` surfaces them | ✅ orchestrator + MCP |
| 21 — envelope `requirement_findings[]`; values redacted | ✅ orchestrator + audit |
| 22 — malformed fail-closed; absent = status quo; caps bound count + pointers | ✅ unit + repository |
| Determinism property (same plan + config → same finding set) | ✅ unit |

## Test plan

- `cargo test --workspace` — **all green** (2055 engine lib tests + integration).
- New: `engine/tests/unit/sequence-policy/step-requirement/` (AC 18–22 +
  determinism) and orchestrator integration tests (identity refuse, params
  deny/allow, promote + envelope evidence, plan-preview finding).
- MCP: `rigorix_validate_plan` surfaces `requirement_findings`.
- `cargo clippy --workspace --all-targets -- -D warnings` clean;
  `cargo fmt --check` clean.
- Engine `local-ci.sh`: **68/69** (the one failure is the pre-existing
  `stage_remaining.sh` "No runbook.md found" path lookup, unrelated).

## Follow-ups (out of scope, per ADR-015)

- rigorix-sdk `schemas/policy.json` additive `stepRequirement` /
  `requirementAction` + conformance (separate SDK issue). This PR is
  serializer-compatible (empty `requirements` is omitted), so existing
  conformance stays green.
- Required-companion-step obligations.
- Enterprise policy-bundle ingestion.

Refs #884 · Refs ADR-015 · GAP-A-28
