---
guardian_issue:
  id: "ISSUE-EFFECT-ORACLE-STILL-UNWIRED"
  title: "approval: the effect-scope oracle still ships disabled in all three entry points — #1072 closed COMPLETED with the wiring half deferred"
  epic: "authority boundary"
  component: "Approval"
  module: "approval"
  status: planned
  priority: high
  dependencies:
    - "#1072 — closed COMPLETED; this is the half that remains"

  intent: |
    The staged-changes and per-loop-baseline halves of #1072 were fixed, and the module now documents
    the per-loop limit honestly. The wiring half was not: every shipped entry point still leaves
    \`ParallelExecutionFactoryConfig::approval_repo_path\` at \`None\`, so the oracle produces no
    evidence at all in the default. The issue that named this was closed COMPLETED with no comment
    recording the deferral, which is why it should be re-filed rather than re-opened: the remaining
    work is a decision about the default, not a bug in the oracle.

  in_scope:
    - "Decide and record the default: wire a repo path where one is available, or make the absence visible"
    - "If left optional, the engine must say so at the point a binding is armed without a repo path"
    - "Correct the record: note in #1072 that the wiring half was deferred, so the closure does not read as complete"

  out_of_scope:
    - "Content-level effect detection (paths, not content, is a documented limit)"
    - "Per-step attribution (per-loop baseline is documented)"
    - "Making the check blocking (R2 is the blocking check by design)"

  affected_layers:
    domain:
      - "approval/infrastructure/effect_scope.rs — GitDiffEffectOracle"
    application:
      - "execution_engine/application/service_impl/dispatch.rs:229,423 — effect_scope_check"
      - "execution_engine/application/factory.rs — approval_repo_path"
    infrastructure:
      - "cli, mcp, actions — the three composition roots"
    api:
      - "None"

  canonical_references:
    - evidence: "approval/infrastructure/effect_scope.rs:24-30 — 'Not wired by default. Every shipped entry point leaves approval_repo_path at None (CLI, GitHub Action, MCP host)'"
    - evidence: "dispatch.rs:229 — the check runs only when 'requires_approval' and both 'scope_baseline' and 'approval_repo_path' are Some"
    - evidence: "dispatch.rs:423 — effect_scope_check is non-blocking; it forwards violations to the binding"
    - evidence: "#1072 closed 2026-10-18 as COMPLETED with no comment recording a deferral"

  acceptance_criteria:
    - "A recorded decision: either a default repo path, or a stated reason the default is None"
    - "When an approval binding is armed and no repo path is configured, the engine emits a warning naming the consequence (effect scope is not verified) — today the silence is identical to a clean run"
    - "The chosen behaviour has a test that fails if the warning or the wiring is removed"
    - "The module doc and the decision agree; no doc may imply effect-scope coverage in the default"

  validators:
    - architecture
    - integration

  implementation_notes: |
    The honest framing is that an unwired safety control and an absent one are indistinguishable at
    the point of use. A run with no repo path produces exactly the output of a run whose effects
    stayed in scope, so a reader cannot tell refusal from silence. That is the whole reason this is
    high priority despite the oracle itself being correct.

    Do not wire a default by guessing a repo root. \`approval_repo_path\` is a string the operator
    supplies; inventing one would make the check run against the wrong tree, which is worse than not
    running it.

  file_changes:
    - "modify: engine/src/execution_engine/application/service_impl/dispatch.rs"
    - "modify: engine/src/approval/infrastructure/effect_scope.rs"
    - "modify: engine/src/execution_engine/application/factory.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-EFFECT-ORACLE-STILL-UNWIRED

## Verified state

| element | state |
|---|---|
| oracle exists | \`GitDiffEffectOracle\`, 263 lines, 3 tests |
| staged changes counted | fixed — \`staged_changes_are_part_of_the_snapshot\` |
| wired in CLI / Actions / MCP | **no** — \`approval_repo_path\` is \`None\` in all three |
| path-level (not content) | documented limit |
| per-loop baseline | documented limit |

The module says it plainly:

> **Not wired by default.** Every shipped entry point leaves \`ParallelExecutionFactoryConfig::approval_repo_path\`
> at \`None\` (CLI, GitHub Action, MCP host), so no effect-scope coverage is produced in the OSS default.

## Why silence is the problem

\`effect_scope_check\` returns early when the binding is absent, when there is no declared scope, or
when the snapshot fails. In every one of those cases the run completes with the same output as a run
in which the effects were inspected and found in scope. An operator reading the result cannot
distinguish "verified clean" from "never checked".

That is the class this whole series has been chasing: a control that can be off looks identical to a
control that is on.

## What #1072's closure hides

#1072 was closed as \`COMPLETED\` with no comment. Two of its three parts were fixed and are verifiable
in the code. The third — "disabled in every production entry point" — is still true, and the issue
title said so. Closing it without recording the split makes the tracker assert something the code
contradicts.

The fix for the tracker is the same as the fix for the code: say which half landed.
