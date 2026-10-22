---
guardian_issue:
  id: "ISSUE-APPROVE-CANNOT-DECLARE-SCOPE"
  title: "the approve surface cannot declare a scope, so run_command steps have no way to be verified — the runbook says the approve surface may set one"
  epic: "consequence gating"
  epic_id: "EPIC-CONSEQUENCE-GATING"
  tracking_issue: 1190
  component: "Approval"
  module: "approval"
  status: planned
  priority: medium
  created_at: "2026-10-23T19:00:00Z"
  updated_at: "2026-10-23T19:00:00Z"

  intent: |
    Let the approver declare the effect scope, so tools the planner cannot scope are still verifiable.

    `#1175` gave the planner a producer for path-shaped tools (`file_write`, `file_append`,
    `file_patch`, `git_stage`). `run_command` deliberately declares nothing — a command string is not a
    path list — and that is correct.

    But `ApproveInput` (`approval/application/dto/mod.rs:28-45`) has no `declared_scope` field:
    `dag_id`, `step_names`, `approver_id`, `authority`, `decision_context`, identity claims. So the one
    party who *knows* which paths a command will touch — the human approving it — cannot say so, and
    the runbook's own sentence ("declared in the node's resolved payload … e.g. set by the planner **or
    the approve surface**", `intent.rs:76-79`) describes an input that does not exist.

    Consequence: for command-based steps, effect detection can never run. Absent a producer, the
    oracle's early return is the only behaviour possible, and `run_command` is the dominant side-effecting
    tool in practice.

  dependencies:
    - name: "ApproveInput"
      type: internal
      note: "approval/application/dto/mod.rs:28-45 — no scope field."
    - name: "ExecutionIntent.declared_scope"
      type: internal
      note: "approval/domain/intent.rs:46 — present; populated from the node payload only."
    - name: "ISSUE-ENVELOPE-UNCHECKED-VS-CLEAN (#1189)"
      type: internal
      note: "Records the outcome; this issue supplies the input that makes `checked` reachable for commands."
    - name: "the approval HTTP/MCP surface"
      type: internal
      note: "Whatever serves approve must accept and validate the field."

  in_scope:
    - "Add `declared_scope: Vec<String>` to `ApproveInput` (optional, defaulted empty)"
    - "Merge precedence: an approver-declared scope wins for the approved steps; planner-declared stays for path tools"
    - "Validate entries (relative, no globs escaping the repo root, no absolute paths) before they reach the intent hash"
    - "Surface it on the approve API and MCP tool that already exists"
    - "Optional strict mode: refuse to approve a step that mutates files with no declared scope"

  out_of_scope:
    - "Deriving a scope from a command string (explicitly not attempted — see notes)"
    - "Blocking violations"
    - "The envelope marker (#1189)"

  affected_layers:
    domain:
      - "Existing: ExecutionIntent.declared_scope — now also supplied by approval"
    application:
      - "Modified: ApproveInput, build_record merge, validation"
    infrastructure:
      - "None"
    api:
      - "Modified: the approve request schema (additive, optional)"

  canonical_references:
    - module: ".pi/architecture/modules/approval.md"
    - adr:
        - ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-23"
    - evidence: "approval/application/dto/mod.rs:28-45 — ApproveInput has no declared_scope"
    - evidence: "approval/domain/intent.rs:76-79 — 'set by the planner or the approve surface'"
    - evidence: "planning/application/pipeline_impl.rs:316-368 — the planner producer (#1175)"
    - evidence: "execution_engine/.../dispatch.rs:478-490 — the oracle needs a non-empty scope"

  acceptance_criteria:
    - "An approver can supply a scope for a `run_command` step and the oracle then verifies it"
    - "Absent, the behaviour is unchanged (`not_declared`, per #1189)"
    - "Invalid entries (absolute, escaping, glob) are refused at approval, not at dispatch"
    - "The approval hash covers the approver-supplied scope (it is part of the intent)"
    - "Strict mode, if implemented, refuses rather than warns"
    - "validate-ci.sh, validate-tests.sh, validate-security.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - security

  implementation_notes: |
    This is the honest source of truth for a command's scope: the person who approved it. Do **not**
    attempt to parse a shell command into paths — the failure mode is a guessed scope, and guessed
    scopes generate false violations, which teach operators to ignore the control.

    Validation matters more than usual because the value lands in the signed intent hash: reject
    absolute paths, `..` escapes, and globs that leave the repository. A scope that can name `/etc` is
    not a scope.

    Default empty and defaulted-on-the-wire keeps every existing approval valid. Strict mode is the
    opt-in that makes the control mandatory for a team that wants it; do not make it the default while
    the producer covers only four tools.

  file_changes:
    - "modify: engine/src/approval/application/dto/mod.rs"
    - "modify: engine/src/approval/application/service_impl.rs (merge + validation)"
    - "modify: the approve HTTP handler and MCP tool schema"
    - "create: engine/tests/approve_declared_scope.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1190: the approver cannot say what the command may touch

## What you are looking at

| tool | scope produced by | verifiable |
|---|---|---|
| `file_write`, `file_append`, `file_patch`, `git_stage` | planner (#1175) | ✅ |
| `run_command` | **nobody** | ❌ |

The runbook says the approve surface may declare a scope; `ApproveInput` has no such field.

## In Scope

- Optional `declared_scope` on approval, validated, merged with the planner's, covered by the hash
- Optional strict mode

## Out of Scope

- Parsing commands into paths; blocking violations; the envelope marker (#1189)

## Affected Layers

### Domain
- `ExecutionIntent.declared_scope` gains a second source

### Application
- `ApproveInput`, `build_record`, validation

### API
- Additive optional field on the approve surface

## Canonical References

- **Module:** `.pi/architecture/modules/approval.md`
- **ADR-011**
- **Evidence:** `dto/mod.rs:28-45`; `intent.rs:76-79`; `pipeline_impl.rs:316-368`; `dispatch.rs:478-490`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Approver-supplied scope ⇒ oracle verified | `validate-integration.sh` |
| 2 | Absent ⇒ unchanged | `validate-tests.sh` |
| 3 | Invalid entries refused at approval | `validate-security.sh` |
| 4 | Hash covers the scope | `validate-tests.sh` |
| 5 | Strict mode refuses, not warns | `validate-tests.sh` |

## Implementation

1. Read `dto/mod.rs:28-45`, `service_impl.rs:70-110`, `dispatch.rs:470-500`.
2. Add the field; validate; merge; ensure the hash covers it.
3. Surface it; test with a `run_command` step.

## Pitfalls

- **Never parse a command into paths** — guessed scopes produce false violations.
- **Validate before hashing**: the value is signed, so bad input becomes bad evidence.
- **Keep the default empty**; strict mode is opt-in until the producer covers more tools.

## Why this is medium

`run_command` is where most real effects happen, and today there is no mechanism by which its scope
could ever be declared — so the control is structurally unavailable for the dominant tool.
