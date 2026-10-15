---
guardian_issue:
  id: "ISSUE-README-RUNNABLE-DEMO-RESTORE"
  title: "README: the runnable demo vanished in the docs rewrite — restore a one-command path"
  epic: "documentation"
  component: "Docs"
  module: "docs"
  status: planned
  priority: high
  dependencies:
    - "PR #1014 (docs rewrite, merged)"

  intent: |
    The docs rewrite made the README traceable to code and removed unverifiable claims — both
    correct — but it also removed the runnable demo section. A reader can no longer see the
    product happen. Restore a one-command, verifiable path to a demo, and give the deferred
    demo visual a place where a reader (not only a maintainer) can see the promise.

  in_scope:
    - "Restore a runnable demo section in README.md with exact commands and expected output"
    - "Point at a demo that exists and runs (validate the commands, do not describe them)"
    - "Add the deferred demo visual where the promise is visible to a reader"
    - "Keep every claim traceable; the restored section must not reintroduce non-derivable numbers"

  out_of_scope:
    - "Building a new flagship demo (separate work)"
    - "Rewriting the rest of the README"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: README.md"
    api:
      - "None"

  canonical_references:
    - guard: ".pi/scripts/validate-docs.sh (gating)"
    - precedent: "authority-demo (Docker-free: one command, three minutes)"

  acceptance_criteria:
    - "Every command in the restored section is executed as part of the change and its output recorded"
    - "validate-docs.sh passes"
    - "The section names the actual demo directory and the actual entry command"
    - "No numeric claim appears that is not derivable from the run or from source"
    - "The demo referenced is Docker-free so the quickstart does not require a container runtime"

  validators:
    - ci
    - tests
    - canonical

  implementation_notes: |
    - The authority-demo property worth preserving is "one command, three minutes" with no
      docker-compose. Do not restore a demo that needs containers.
    - Run the commands before writing them down. The previous version described commands that
      drifted from the code — that was the original complaint.

  file_changes:
    - "modify: README.md"
    - "create: docs/assets/demo/ (visual, if produced)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-README-RUNNABLE-DEMO-RESTORE

## Intent

Restore the conversion path that the docs rewrite removed, without reintroducing unverifiable
claims.

## Why

#1014 correctly made docs traceable to code and correctly deleted unverifiable numbers. It also
deleted the runnable demo. A product README that proves nothing and shows nothing does not
convert, no matter how accurate it is.

## In Scope

A verified, one-command demo section, plus a place for the demo visual.

## Out of Scope

A new flagship demo; the rest of the README.

## Acceptance Criteria

Commands executed and output recorded; no non-derivable numbers; Docker-free.

## Notes

The failure to avoid is describing commands rather than running them — that is how the original
drift happened.
