---
guardian_issue:
  id: "ISSUE-ENFORCEMENT-REFUSAL-SURFACE-INCONSISTENT"
  title: "the --enforcement refusal renders with 18-space gaps, and the comment beside the working path still says the flag selects a preset"
  epic: "enforcement correctness"
  epic_id: "EPIC-ENFORCEMENT-CORRECTNESS"
  tracking_issue: 1171
  component: "Cli"
  module: "cli-boundary"
  status: implemented
  priority: low
  created_at: "2026-10-23T12:00:00Z"
  updated_at: "2026-10-23T12:00:00Z"

  intent: |
    Make the refusal read as prose, and make the code around it describe the behaviour that now
    exists.

    Three small inconsistencies were introduced by the #1156 fix — the same class it was meant to
    close (a surface saying something the code does not do), just one layer down:

    1. **The message has 18-space runs.** The format string was wrapped across source lines without
       `\` continuations, so the operator sees:
       "`--enforcement strict` is not implemented: the executor config is built once
       …18 spaces… at startup". `DispatchResult::error` stores the string verbatim, so the gaps
       render.
    2. **A comment beside the working path still claims the flag works.**
       `orchestrator.rs:405` says "`--enforcement` selects a preset; the standard limits apply
       otherwise" — but the flag is refused, and the preset comes from `[enforcement]` in the config.
    3. **A dead assignment remains.** `dispatch.rs:233` still passes
       `enforcement_preset: enforcement`, which is now provably `None` at that point because the
       `Some` case returned early.

    (1) is the one that matters: it is the entire operator-facing output of the fix, and the fix's
    stated purpose was an *actionable* message.

  dependencies:
    - name: "#1156 refusal"
      type: internal
      note: "dispatch.rs:917-935 — the early return and the format string."
    - name: "DispatchResult::error"
      type: internal
      note: "dispatch.rs:73-79 — stores the summary verbatim; no trimming."
    - name: "the refusal test"
      type: external
      note: "Asserts exit 2 + 'not implemented' by substring, so it passes despite the gaps."

  in_scope:
    - "Collapse the message to single spaces (one literal, or backslash-continued lines)"
    - "Correct the orchestrator.rs:405 comment to name `[enforcement] preset`"
    - "Remove the dead `enforcement_preset: enforcement` assignment, or comment why it stays"
    - "Tighten the test to assert the message contains no double spaces"

  out_of_scope:
    - "Implementing the per-run override"
    - "Removing `RunInput::enforcement_preset` or `with_enforcement_preset` — they are reserved for
      that work and the module docs say so"
    - "Changing the exit code"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "None"
    api:
      - "Modified: the refusal message text; a comment; one dead assignment in the run path"

  canonical_references:
    - module: ".pi/architecture/modules/cli-boundary.md"
    - adr:
        - ".pi/architecture/decisions/ADR-008-budget-enforcement.md"
    - last_sync: "2026-10-23"
    - evidence: "cli/src/cli_boundary/dispatch.rs:931 — the format string with two 18-space runs"
    - evidence: "cli/src/cli_boundary/dispatch.rs:73-79 — summary stored verbatim"
    - evidence: "cli/src/cli_boundary/orchestrator.rs:405 — '--enforcement selects a preset'"
    - evidence: "cli/src/cli_boundary/dispatch.rs:233 — enforcement_preset: enforcement, unreachable as Some"
    - evidence: "cli/src/cli_boundary/orchestrator.rs:407 — enforcement_config_from_engine(&engine_config.enforcement) is the real source"

  acceptance_criteria:
    - "The refusal message contains no run of two or more spaces"
    - "No comment states that --enforcement selects a preset"
    - "The run path does not pass an enforcement_preset that can only be None, or explains why"
    - "The test fails if the message regains double spaces"
    - "Exit code remains 2; validate-ci.sh and validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - canonical

  implementation_notes: |
    Keep the message one literal string. If it must wrap, use `\` at end of line — Rust string
    continuation removes the newline *and* the following indentation, which is exactly the bug here.

    The test tightening is the durable part: asserting `contains("not implemented")` cannot catch a
    corrupted layout. Assert the absence of two consecutive spaces in the summary, not the exact
    wording, so future edits to the sentence do not fail the test.

    Leave `RunInput::enforcement_preset` and the builder method alone. They are reserved for the
    per-run override, and both module docs now say the flag is refused — removing the DTO field would
    make that future work larger for no present benefit. What should go is the assignment that
    cannot carry a value.

  file_changes:
    - "modify: cli/src/cli_boundary/dispatch.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1171: the refusal's own surface is inconsistent

## What you are looking at

The fix is correct: enforcement still works from `[enforcement] preset`
(`orchestrator.rs:407`), and the flag is refused with exit 2. But the refusal text carries two
18-space runs because the literal was wrapped in source without continuations — and
`DispatchResult::error` (dispatch.rs:73) stores it verbatim, so the gaps are what the operator reads.

Two smaller leftovers in the same commit: a comment at `orchestrator.rs:405` that still says
"`--enforcement` selects a preset", and a dead `enforcement_preset: enforcement` at
`dispatch.rs:233` where the value is provably `None`.

## In Scope

- One-literal message (single spaces)
- Comment corrected
- Dead assignment removed or justified
- Test asserts no double spaces

## Out of Scope

- The per-run override; removing the DTO field/builder (reserved for it); the exit code

## Affected Layers

### API
- The refusal message, one comment, one dead assignment

## Canonical References

- **Module:** `.pi/architecture/modules/cli-boundary.md`
- **Evidence:** `dispatch.rs:931`, `:73-79`, `:233`; `orchestrator.rs:405`, `:407`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | No run of 2+ spaces in the message | `validate-tests.sh` |
| 2 | No comment claims the flag selects a preset | `validate-canonical.sh` |
| 3 | Dead assignment gone or justified | review |
| 4 | Exit code still 2 | `validate-tests.sh` |

## Implementation

1. Read `dispatch.rs:917-940`, `:70-80`, `:225-240`; `orchestrator.rs:398-412`.
2. Collapse the literal; fix the comment; drop the dead assignment.
3. Tighten the test to reject double spaces.

## Pitfalls

- **Use `\` continuation, not naive wrapping** — that is what produced this.
- **Do not delete the DTO field**; it is reserved for the per-run override.
- **Assert the property (no double spaces), not the sentence**, or the test becomes a maintenance
  tax on wording.

## Why this is low

Nothing behaves incorrectly. But the one output the fix exists to produce renders with gaps
mid-sentence, and a comment next to the working path still describes the behaviour that was removed.
