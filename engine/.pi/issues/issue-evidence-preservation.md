---
guardian_issue:
  id: "ISSUE-EVIDENCE-PRESERVATION"
  title: "Evidence preservation: approval-resume re-emission drops ALL finding arrays (ADR-013/015/017)"
  epic: "precondition-followups"
  epic_id: "EPIC-PRECONDITION-FOLLOWUPS"
  component: "AuditEvidenceComposition"
  module: "audit"
  status: planned
  priority: critical
  dependencies:
    - "ADR-011 / ADR-013 / ADR-015 / ADR-017"
    - "ISSUE-PF-ACT-1/2 (gate wiring, merged)"

  intent: |
    An approval-gated run loses ALL signed finding evidence. The engine writes
    a finding-bearing envelope, then the MCP host OVERWRITES it with a final
    envelope synthesized from `node_states` (node_completed/node_failed +
    approval_recorded), which never contains the finding events. Result:
    `precondition_findings[]` (ADR-017), `sequence_policy_findings[]`
    (ADR-013) and `requirement_findings[]` (ADR-015) are absent for the
    flagship "approved at T0, refused at Tn" path — and `rigorix_read_audit`
    cannot surface them at all. Fix the composition so the signed record
    matches the run, for all three finding kinds, on both the persisted engine
    envelope and the MCP audit-read surface.

  in_scope:
    - "MCP host: build the final engine envelope from the REAL event stream (not only node_states)"
    - "MCP audit-tools: model + populate the finding arrays so rigorix_read_audit surfaces them"
    - "Regression tests at the composition boundary (run -> approve -> assert evidence)"
    - "HMAC/canonicalization + formatter updates for the new audit-tools fields"

  out_of_scope:
    - "New finding types or schema changes (the envelope fields are already frozen)"
    - "The SDK verifier / anchor contract (unchanged; fields already exist)"
    - "Changing the enforcement semantics (only the evidence path)"

  affected_layers:
    api:
      - "Modify: mcp/src/host/mod.rs (final-envelope re-emission; 570-645, build_envelope_from_run 843)"
    domain:
      - "Modify: mcp/src/audit_tools/domain/value.rs (AuditEnvelope model)"
    infrastructure:
      - "Modify: mcp/src/audit_tools/infrastructure/in_memory_audit_service.rs (build_from_run, compute_hmac)"
    tests:
      - "New: mcp/tests/ (or engine/tests/) composition regression for finding evidence"

  canonical_references:
    - adr: "engine/.pi/architecture/decisions/ADR-017-consequence-gating.md"
    - adr: "engine/.pi/architecture/decisions/ADR-013-sequence-policy.md"
    - adr: "engine/.pi/architecture/decisions/ADR-015-operator-step-requirements.md"
    - module: "engine/.pi/architecture/modules/precondition.md#evidence"
    - code: "engine/src/audit/domain/envelope.rs (finding fields, ~230-266)"
    - code: "engine/src/audit/application/envelope_factory_impl.rs (derivation, 159-161, 529/579/628)"
    - code: "mcp/src/host/mod.rs:570 (override)"
    - code: "engine/src/orchestrator/application/orchestrator_impl.rs:634 (initial write)"

  acceptance_criteria:
    - "AC1: an approval-gated run whose step is refused by a precondition persists `.rigorix/audit/<id>.json` with `precondition_findings[0].outcome='failed'` incl. exit_code, inputs_hash, checked_at"
    - "AC2: the same for a sequence-policy denial → `sequence_policy_findings[]` present"
    - "AC3: the same for a requirement (unmet/promoted) → `requirement_findings[]` present"
    - "AC4: `rigorix_read_audit` returns the finding arrays for that run"
    - "AC5: the persisted envelope still verifies (HMAC/chain) and the cross-language fixtures are unchanged"
    - "AC6: non-approval runs are unchanged (findings still present) — no regression"
    - "AC7: a composition regression test fails on the pre-fix code and passes after"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Root cause. The MCP host re-emits a FINAL engine envelope after a run by
    SYNTHESIZING events from `state.node_states` (mcp/src/host/mod.rs ~570-645)
    — only `node_completed`/`node_failed` + `approval_recorded`. The engine's
    finding fields are derived from events by
    `envelope_factory_impl::{precondition,sequence_policy,requirement}_findings_from_events`
    (lines 159-161, 529/579/628); with the synthesized event list, every
    finding array is empty and `#[serde(skip_serializing_if = "Vec::is_empty")]`
    omits it. The orchestrator writes the correct envelope on the INITIAL run
    (orchestrator_impl.rs:634, real `record.events`), but the approval RESUME
    path (`approve_execution_flow`) writes no envelope, so the MCP synthesis is
    the only writer — and it is lossy.

    Repro (installed 1.9.0, identical check, only approval differs):
      - direct run (no `requires_approval`): envelope events ["precondition_checked"],
        precondition_findings = [{outcome:failed, exit_code:3, inputs_hash:sha256:…, checked_at, summary}]
      - approval-gated run: envelope events ["node_failed"], precondition_findings absent.

    Fix options (recommend 1 + 3 together):
    1. Capture the real events. The engine event bus is `tokio::sync::broadcast`
       (`EventBusServiceImpl`; `subscribe_receiver()` / `raw_subscribe()`). Have
       the MCP host subscribe for the execution before dispatching a run AND
       before calling `approve_execution`, buffer `ExecutionEvent`s for the
       execution_id, convert to `ExecutionEventRef`, and build the final
       `BuildEnvelopeInput.events` from them (merged with the synthesized
       node/approval events, de-duplicated). This preserves every finding kind
       with no schema change.
    2. (Cleaner long-term) make the engine the single writer: have
       `OrchestratorImpl::approve_execution_flow` emit the final envelope from
       the real `record` events and delete the MCP synthesis entirely.
    3. MCP audit-tools model. `mcp/src/audit_tools/domain/value.rs::AuditEnvelope`
       has no finding fields at all — so `rigorix_read_audit` is structurally
       incapable of showing them. Add `precondition_findings`,
       `sequence_policy_findings`, `requirement_findings` (mirroring
       `engine/src/audit/domain/envelope.rs`), populate them in
       `build_from_run`/`build_envelope_from_run`, include them in
       `compute_hmac` canonicalization, and render them in the formatter.

    Do NOT change the envelope wire schema (already frozen) or the SDK fixtures.
    Keep SpanPrivacy: no parameter values, no raw stdout.

  file_changes:
    - "modify: mcp/src/host/mod.rs"
    - "modify: mcp/src/audit_tools/domain/value.rs"
    - "modify: mcp/src/audit_tools/infrastructure/in_memory_audit_service.rs"
    - "modify: mcp/src/audit_tools/domain/formatter_impl.rs"
    - "create: mcp/tests/evidence_preservation_test.rs (or engine/tests/)"
---

# ISSUE-EVIDENCE-PRESERVATION

## Problem

The **approval-gated** path — the flagship "approved at T₀, refused at Tₙ" scene —
drops every signed finding array:

| Run shape | Envelope `events` | `precondition_findings[]` |
|---|---|---|
| Direct (no approval) | `["precondition_checked"]` | ✅ present (outcome, exit_code, inputs_hash, checked_at) |
| Approval-gated | `["node_failed"]` | ❌ **absent** |

Same for `sequence_policy_findings[]` (ADR-013) and `requirement_findings[]`
(ADR-015). The conference-demo's approval scene already shows
`sequence_policy_findings: None`; it was never asserted, so it went unnoticed.

## Why this was missed (process diagnosis)

1. **Every assertion lives on the engine/orchestrator path, not the MCP
   composition path.**
   - `orchestrator_impl/tests/policies.rs:372` asserts
     `envelope.sequence_policy_findings.len() == 1`;
     `tests/approval.rs:401` asserts `envelope.requirement_findings.len() == 1`.
     Both call the **engine** envelope builder, which receives the real
     `record.events` — so they pass.
   - `envelope_factory_impl` tests feed **synthetic** events; they prove the
     derivation, not that the composition supplies the events.
2. **The MCP tests assert the wrong surface.** `mcp/src/execution_tools/tests.rs`
   checks plan-time `validate_plan` findings — not the persisted envelope or
   `rigorix_read_audit`.
3. **Two writers, each tested in isolation.** The engine writes a
   finding-bearing envelope; the MCP host **overwrites** it with a synthesized
   one. The re-emission was added for a different bug ("pause snapshot must not
   stand in for a completed run", F-20260907-05) and its regression test only
   asserted node/side-effect outcomes — never the finding arrays. The
   **composition** was untested.
4. **The MCP audit-tools model never had the fields.** `AuditEnvelope`
   (`mcp/src/audit_tools/domain/value.rs:42`) has no finding arrays, so
   `rigorix_read_audit` could never show them — no test could catch a field
   that does not exist.
5. **The demos tolerated absence.** `conference-demo/.rigorix/run-conference-demo.mjs:362`
   *prints* `sequence_policy_findings=${(env.sequence_policy_findings ?? []).length}`
   but never asserts `> 0`; payouts/migration demos don't reference the fields.
   The demos asserted **behavior** (did it pause? did the step run? DB state),
   not **evidence content**.
6. **No validator checks evidence content.** Docs/CHANGELOG claim the fields;
   the canonical/architecture validators check docs/code presence, not that a
   run's envelope actually carries them.

**Class:** unit-green, integration-blind — the feature was asserted at the layer
that *has* the data, never at the composition boundary that *drops* it.

## Fix design

1. **Preserve the real events (MCP host).** The engine bus is
   `tokio::sync::broadcast` (`EventBusServiceImpl`). Subscribe before a run and
   before `approve_execution`; buffer `ExecutionEvent`s for the execution_id;
   build the final envelope from them (merged with node/approval events).
2. **(Preferred long-term) single writer.** Make
   `OrchestratorImpl::approve_execution_flow` write the final envelope from the
   real `record` events, and delete the MCP synthesis.
3. **Model the fields (MCP audit-tools).** Add the three finding arrays to
   `AuditEnvelope`, populate them, include them in the HMAC, render them.

## Acceptance criteria

| # | Criterion | Verify |
|---|-----------|--------|
| 1 | Approval-gated precondition refusal → persisted envelope has `precondition_findings[0].outcome="failed"` + exit_code + inputs_hash + checked_at | integration test |
| 2 | Approval-gated sequence denial → `sequence_policy_findings[]` present | integration test |
| 3 | Approval-gated requirement unmet/promoted → `requirement_findings[]` present | integration test |
| 4 | `rigorix_read_audit` returns the finding arrays | integration test |
| 5 | Persisted envelope still verifies (HMAC/chain); SDK fixtures unchanged | unit + conformance |
| 6 | Non-approval runs unchanged | integration test |
| 7 | Test fails pre-fix, passes post-fix | regression |

## Non-goals

- New finding kinds or envelope schema changes (fields are frozen).
- SDK/anchor contract changes.
- Enforcement semantics.

## References

- ADR-013 (sequence policy), ADR-015 (requirements), ADR-017 (preconditions)
- `mcp/src/host/mod.rs:570-645` (lossy re-emission), `:843` (`build_envelope_from_run`)
- `mcp/src/audit_tools/domain/value.rs:42` (model missing fields)
- `engine/src/audit/domain/envelope.rs:263` (fields exist), `envelope_factory_impl.rs:159-161`
- `engine/src/orchestrator/application/orchestrator_impl.rs:634` (initial write), `:1394` (approve writes none)
- `engine/src/event_system/application/event_bus_service_impl.rs` (broadcast bus)
