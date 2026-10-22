---
guardian_issue:
  id: "ISSUE-WIRING-CLAIMS-AND-STATUSES-STALE"
  title: "the OSS wiring-claims manifest has no effect-commitment row, and three shipped modules still say Status: Planned"
  epic: "architecture readiness"
  epic_id: "EPIC-ARCHITECTURE-READINESS"
  tracking_issue: 1177
  component: "Docs"
  module: "effect-commitment"
  status: planned
  priority: medium
  created_at: "2026-10-23T15:00:00Z"
  updated_at: "2026-10-23T15:00:00Z"

  intent: |
    Add the missing claim row and correct the module statuses, so the governance artifacts describe
    the code that exists.

    Two independent staleness problems, both in the same class - an artifact that under- or
    over-claims relative to the code:

    1. **`engine/.pi/architecture/wiring-claims.md` has no effect-commitment row.** The manifest is the
       one place that records "this module's wiring is verified, here is how". It has 12 rows (W1-W12
       plus notes) and covers approval, sequence policy, preconditions and the console. Effect
       commitment - write-ahead before dispatch, settle after, startup reconcile in three roots - is
       absent, so the context added in this release is the only significant seam with no claim.
    2. **Three shipped modules still say `Status: Planned`**: `recovery-recipes`, `hooks`,
       `policy-engine`. All three have source trees, all three are exercised by the issues that just
       closed (#1157 touched `recovery_recipes`), and `hooks` carries a production fix (`HookRunner`
       BrokenPipe handling). The same under-claim exists on the enterprise side (#292) — this is the
       OSS half of a pattern, not a one-off.

  dependencies:
    - name: "wiring-claims.md"
      type: internal
      note: "engine/.pi/architecture/wiring-claims.md — the manifest; W6 already covers the scope oracle."
    - name: "module docs"
      type: internal
      note: "engine/.pi/architecture/modules/{recovery-recipes,hooks,policy-engine}.md — status lines."
    - name: "ISSUE-NOTIFICATIONS-STATUS-UNDERCLAIMS (#292, enterprise)"
      type: external
      note: "The same under-claim class, already filed on the enterprise side."

  in_scope:
    - "Add effect-commitment rows: the write-ahead seam, the settle seam, and the startup reconcile in all three roots"
    - "Each row's evidence must be checkable (path + how to verify), in the manifest's existing style"
    - "Correct the three module status lines to what the code and tests show"
    - "State the audit method in the manifest header so the next row is added the same way"

  out_of_scope:
    - "The enterprise wiring-claims manifest (separate repo, its own guard)"
    - "Promoting modules without evidence"
    - "The eight Planned modules that genuinely are not implemented (`permission-enforcer`, `quality-gates`, …)"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "engine/.pi/architecture/wiring-claims.md — 12 rows, none for effect-commitment"
    - evidence: "effect_commitment dispatch seam: dispatch.rs begin_effect before spawn_concurrent_node; settle_effect after"
    - evidence: "setup.reconcile() called at cli/orchestrator.rs:380, mcp/host/mod.rs:1616, actions/main.rs:433"
    - evidence: "modules/recovery-recipes.md, hooks.md, policy-engine.md — '**Status:** Planned' with source trees present"
    - evidence: "hooks/application/hook_runner_impl.rs:117-120 — the BrokenPipe fix, in a module marked Planned"

  acceptance_criteria:
    - "wiring-claims.md has effect-commitment rows with checkable evidence"
    - "The three module statuses match the code"
    - "No module is promoted without a test or a call site as evidence"
    - "The manifest states how a row is verified"
    - "validate-ci.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - canonical

  implementation_notes: |
    The rows matter more than the statuses. A status line is a claim; a manifest row is a claim plus a
    procedure for checking it, and the manifest is what the enterprise guard reads. Write the three
    rows the way W6 is written: component, module, claim, verdict, evidence.

    For effect-commitment the checkable facts are: `begin_effect` runs before
    `spawn_concurrent_node` in both dispatch sites; `settle_effect` runs on the terminal outcome;
    `reconcile()` is called in all three roots and fails closed; absent config arms nothing. Each is a
    line of code plus a test name.

    Do not turn this into a status audit of every module. Fix the three that are demonstrably shipped
    and record in the manifest header how to decide the next one, so the pattern does not need a
    future issue.

  file_changes:
    - "modify: engine/.pi/architecture/wiring-claims.md"
    - "modify: engine/.pi/architecture/modules/recovery-recipes.md"
    - "modify: engine/.pi/architecture/modules/hooks.md"
    - "modify: engine/.pi/architecture/modules/policy-engine.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1177: governance artifacts lag the shipped context

## What you are looking at

| artifact | state |
|---|---|
| `wiring-claims.md` | 12 rows, **none for effect-commitment** |
| `recovery-recipes.md` | **Planned** — source tree present, fixed by #1157 |
| `hooks.md` | **Planned** — source tree present, carries the BrokenPipe fix |
| `policy-engine.md` | **Planned** — source tree present, used on the ingest path |

## In Scope

- Three effect-commitment rows with checkable evidence
- Three status corrections
- A stated method for adding the next row

## Out of Scope

- The enterprise manifest; promoting without evidence; the genuinely-unimplemented modules

## Affected Layers

### API
- None — documentation artifacts

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **ADR-019**
- **Evidence:** `wiring-claims.md`; the three module status lines; the dispatch and reconcile call sites

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | effect-commitment rows present, evidence checkable | `validate-canonical.sh` |
| 2 | Three statuses corrected | `validate-canonical.sh` |
| 3 | No unsubstantiated promotion | review |
| 4 | Method stated in the manifest header | review |

## Implementation

1. Read `wiring-claims.md` (all rows) and the effect-commitment dispatch/reconcile sites.
2. Add the rows in W6's shape.
3. Correct the three statuses with evidence in the MR description.
4. Add the method note.

## Pitfalls

- **Under-claiming is as wrong as over-claiming** — it removes a module from the audit, which is what
  the enterprise guard keys on.
- **Do not bulk-update statuses**; three with evidence, and a method for the rest.
- **Evidence must be a path plus a check**, not "it works".

## Why this is medium

The manifest is the audit surface, and the newest boundary is not on it. A status line saying Planned
for a module that ships is the same defect from the other direction: it makes the code invisible to the
guard that reads status lines.
