# Runbook: sequence-policy Module

<!--
Canonical Reference: .pi/architecture/modules/sequence-policy.md
Last Updated: 2026-09-05
-->

## Overview

The `sequence-policy` module detects **composed abuse across ordered steps** —
actions individually permitted but collectively outside operator intent (the
"remove-then-reassign" conference class). Rules are declarative, deterministic,
operator-authored (`no LLM judgment in the enforcement path`), and evaluated at
two choke points:

1. **Plan time (R2, primary)** — the ordered runbook/template step list is
   evaluated before anything executes (`orchestrator.run_from_template` /
   `plan_from_template`). A matched `promote` rule builds the later step
   `requires_approval = true` (existing approval pause); a matched `deny` rule
   refuses the runbook (fail closed, tool never called).
2. **Run time (R3, fallback)** — for dynamic plans whose steps are not all
   pre-materialized, `run_dispatch_loop` evaluates the session's completed
   dispatch prefix + the next ready node before dispatching it. `promote`
   routes the node into the existing `AwaitingApproval` pause; `deny` fails the
   node with a structured `sequence_policy_denied` before its tool is called.

The defensive property:

> **A forbidden sequence never executes silently.** If an ordered plan contains
> a step pair matching a rule, the later step pauses for a human (promote) or
> is denied outright (deny) — deterministically, recorded into the signed
> envelope (`sequence_policy_findings[]`, redacted by default).

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `SequenceRule` | Domain aggregate | Ordered `StepPredicate` chain (≥2), optional `window`, `action` (`promote` default / `deny`) |
| `StepPredicate` / `ParamPredicate` | Domain value objects | Tool exact/glob match + JSON-pointer param predicates (exact / glob / regex) |
| `Matcher` | Domain/app algorithm | Deterministic windowed matching over ordered step views; earliest-extension, config order |
| `SequenceMatch` | Domain value object | `rule_id`, `action`, `matched_indices`, `later_step` + redacted `decision_summary()` |
| `SequencePolicyConfig` / `SafetyCaps` | Domain config | `fail_closed` (default true) + ordered rules; caps: 100 rules / 8 steps per rule / window 5 / 8 regex predicates per file |
| `SequencePolicyService` / `SequencePolicyServiceImpl` | Application trait/impl | `evaluate_plan` (R2) + `evaluate_prefix` (R3); per-run rule loading |
| `SequencePolicyRepository` / `TomlSequencePolicyRepository` | Infra trait/impl | Reads `.rigorix/sequence-policy.toml`; missing file → `Ok(None)`; corrupt/over-cap → `Err` |
| Orchestrator R2 gate | Choke point | `apply_plan_time_sequence_policy` → promote flips `requires_approval`, deny refuses, publishes `SequenceRuleMatched` evidence |
| Execution-engine R3 gate | Choke point | `sequence_policy_verdict` in `run_dispatch_loop` (promote/deny/config-error events) |

## Dependencies

| Dependency | Purpose | Failure behavior |
|------------|---------|------------------|
| `.rigorix/sequence-policy.toml` | Operator rule file (same trust surface as permissions/policy) | **Missing** → no rules → status quo (fail-open-absent); **corrupt / over cap** → `Err` → run refused at plan time (fail closed) |
| Approval module (ADR-011) | Promote pause/approve/resume machinery | Promoted node waits in `AwaitingApproval`; unchanged semantics |
| Execution engine | R3 dispatch choke point (`run_dispatch_loop`, shared by execute + resume) | Optional `sequence_policy` service; `None` = status quo |
| Permission enforcer | R5 `.rigorix/**` write denial | `.rigorix/**` agent writes denied under `WorkspaceWrite` (operator-only via `DangerousFullAccess`) |
| Audit envelope | R6 `sequence_policy_findings[]` + match/deny/config-error events | Event publish failures are warn-logged (GAP-M-14), never silent |

## Startup Sequence

1. **No module startup is required** — the module is stateless between runs.
   Rules are loaded per run from the operator file through the injected
   repository.
2. **Wiring (factory-built engines)** — attach `with_sequence_policy(svc)` /
   `ParallelExecutionFactoryConfig.sequence_policy` with a
   `SequencePolicyServiceImpl` over a `TomlSequencePolicyRepository`
   (config path: `<repo>/.rigorix/sequence-policy.toml`).
3. **Orchestrator** — `OrchestratorServiceImpl::with_sequence_policy` (R2 gate
   on runbook/template paths; plan preview via `plan_from_template`).
4. **Execution engine** — same service injected at the dispatch loop (R3 gate
   for dynamic plans).
5. **Validate** — `rigorix_validate_plan` surfaces promote findings /
   deny refusal pre-run (MCP surface).

## Graceful Shutdown

The module holds no cross-run state, timers, or background tasks — shutdown is
immediate:

1. **In-flight runs** follow the executor's pause/abort path; a promoted node
   mid-approval remains `AwaitingApproval` in persisted state and resumes after
   restart via the existing hydrate/approve/resume flow (no sequence-policy
   state to rehydrate — rules are re-read per run).
2. **Evidence** — any emitted `SequenceRuleMatched` / `SequencePolicyDenied`
   events already drained into the envelope need no flush on the module side.
3. No special drain, checkpoint, or lock release is module-owned.

## Common Failure Modes and Recovery

| Mode | Symptom | Recovery |
|------|---------|----------|
| **Corrupt rule file** | Plan refused: `SequencePolicyEvaluationFailed` / `SequencePolicyConfigError` (fail closed) | Operator fixes `.rigorix/sequence-policy.toml`; no plan runs under an unparseable rule set (by design). Config errors are non-retriable |
| **Over-cap file** | Same as corrupt (`RuleExceedsCaps`) | Reduce rules / steps / window / regex predicates under the caps |
| **Missing config file** | No gating at all (fail-open-absent, documented) | If gating is expected, create the file — a missing file is **not** an error |
| **Promote rule pauses a run** | Record `PendingApproval`, node `AwaitingApproval` | Human approves (approve → executes) or declines (never dispatched); run stays resumable |
| **Deny rule match (plan)** | `SequencePolicyDenied` refusal before any step | Operator re-authorizes the composition explicitly or amends the rule |
| **Deny rule match (runtime)** | Node fails with `sequence_policy_denied`, tool never called | Same as above; siblings/dependents release like any dispatched failure |
| **R3 eval error mid-dispatch** | Run halts fail-closed before the node dispatches | Operator fixes the config; the halted run surfaces `SequencePolicyConfigError` evidence |
| **Regex ReDoS** | (Prevented) | Safety cap 8 regex predicates/file + compile once per load |
| **Agent edits the rules** | (Prevented) | `.rigorix/**` writes denied to `workspace_write` agents (R5); operator mode only |
| **Param leak into summaries** | (Prevented) | `decision_summary()` redacts by default (SpanPrivacy); full payload opt-in only |

## Configuration Reference

```toml
# .rigorix/sequence-policy.toml — operator-authored. Never agent-writable (R5).
fail_closed = true            # default true; corrupt/over-cap config refuses plans

[[rules]]
id = "registration-remove-then-reassign"
name = "No remove-then-reassign of a full event seat"
description = "Removing an attendee to free a seat, then registering the requester, is never autonomous"
steps = [
  { tool = "registration_remove", params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
  { tool = "registration_add",    params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
]
window = 3                       # max index gap first→last matched step
action = "promote"               # promote (default) | deny
```

**Safety caps (fixed defaults):** `max_rules_per_file = 100`,
`max_steps_per_rule = 8`, `max_window = 5`,
`max_regex_predicates_per_file = 8` (regex count is the ReDoS surface).

## Escalation

- **False pause** (promote fired where the pair is legitimate): the operator
  approves explicitly (human in the loop) — no config change needed.
- **False denial / over-blocking**: adjust the rule (narrow predicates,
  extend `window`, or remove the rule). Rules are deterministic — same input,
  same outcome (property-tested).
- **Rule doesn't fire when it should**: verify step `tool` names / param JSON
  pointers against the predicate contract; remember predicates are ordered and
  `window` caps the gap; check the config actually loaded (missing file is
  fail-open). `rigorix_validate_plan` previews findings pre-run.
