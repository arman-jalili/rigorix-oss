---
guardian_issue:
  id: "ISSUE-W17-CLAIMS-SWEEP-AT-L2"
  title: "W17 says every composition root sweeps orphaned Dispatched records; at L2 none does, and the guard only checks that the evidence path exists"
  epic: "architecture readiness"
  epic_id: "EPIC-ARCHITECTURE-READINESS"
  tracking_issue: 1183
  component: "Docs"
  module: "effect-commitment"
  status: planned
  priority: medium
  created_at: "2026-10-23T16:00:00Z"
  updated_at: "2026-10-23T16:00:00Z"

  intent: |
    Make W17 state the conditional it now has, and give the manifest a way to express "this holds for
    one backend".

    The row (`engine/.pi/architecture/wiring-claims.md:85`):

    > | W17 | `modules/effect-commitment.md` | **every composition root sweeps orphaned `Dispatched`
    > records at startup** | CONFIRMED | `.reconcile()` in CLI, Action **and** MCP host |

    After #1173 that is false for any root configured with `backend.kind = "http"`:
    `HttpEffectLedger::can_sweep_dispatched()` returns `false`, and `EffectReconciler::reconcile`
    returns `Ok(0)` without querying (`reconcile.rs:80-86`), logging that the service owns the sweep.

    Both changes landed in the same batch — #1177 wrote the row (PR #1179), #1173 added the capability
    (PR #1180) — and nothing reconciled them. The row now asserts a property that the code deliberately
    removed for one configuration.

    **The guard cannot catch it.** `validate-wiring-claims.sh` fails a CONFIRMED row when its evidence
    *file or path* disappears (`:109-121`); it does not evaluate the claim's condition. `.reconcile()`
    still appears in all three roots, so W17 passes while the swept-or-not answer depends on the
    backend.

  dependencies:
    - name: "wiring-claims.md W17"
      type: internal
      note: "engine/.pi/architecture/wiring-claims.md:85."
    - name: "can_sweep_dispatched (#1173)"
      type: internal
      note: "interfaces/effect_commitment_ledger.rs:90; in_memory.rs:101 (true); http.rs:204 (false)."
    - name: "reconcile early return"
      type: internal
      note: "application/reconcile.rs:80-86 — returns Ok(0) for a shared ledger."

  in_scope:
    - "Reword W17 to the actual behaviour: every root *calls* reconcile; at L1 it sweeps; at L2 it declines and the service owns it"
    - "Evidence for both halves (the call sites, and the capability returning false for http)"
    - "Add the conditional convention to the manifest header so the next conditional row has a form"

  out_of_scope:
    - "Changing the sweep behaviour (#1173 is correct)"
    - "Teaching the guard to evaluate arbitrary claim semantics — see the note"
    - "The enterprise manifest"

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
    - evidence: "wiring-claims.md:85 — the W17 row"
    - evidence: "interfaces/effect_commitment_ledger.rs:90 — can_sweep_dispatched"
    - evidence: "in_memory.rs:101-104 (true), http.rs:204-208 (false)"
    - evidence: "application/reconcile.rs:80-86 — Ok(0) when the ledger is shared"
    - evidence: "cli/orchestrator.rs:380, mcp/host/mod.rs:1616, actions/main.rs:433 — the call sites"
    - evidence: ".pi/scripts/validate-wiring-claims.sh:109-121 — evidence-existence check only"

  acceptance_criteria:
    - "W17 states the L1/L2 split and is true as written"
    - "Its evidence covers both the call sites and the http capability"
    - "The manifest header documents how to write a backend-conditional row"
    - "validate-wiring-claims.sh passes with the corrected row"
    - "No other row in the manifest asserts an unconditional property that is now conditional"

  validators:
    - architecture-validator
    - ci
    - canonical

  implementation_notes: |
    Do not 'fix' this by deleting the row. The wiring is real at L1, and at L2 the correct statement is
    that the root deliberately declines — which is worth recording, because the next reader will
    otherwise assume the sweep disappeared.

    A short conditional form in the header is cheaper than teaching the guard semantics: for example
    "claim | verdict | evidence — a claim qualified by a backend must name the qualifier and carry
    evidence for each branch". Then W17's evidence names `in_memory: true` and `http: false` explicitly.

    While editing, check the other 16 rows for the same pattern: any row whose truth depends on a
    configuration rather than on the code alone. W15 (`the seam is armed from .rigorix/... at
    composition time`) is about the call, not the configuration, so it is likely fine — but say so
    rather than assume.

  file_changes:
    - "modify: engine/.pi/architecture/wiring-claims.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1183: W17 was true when written and is false for one backend

## What you are looking at

| | L1 (`in_memory`) | L2 (`http`) |
|---|---|---|
| `can_sweep_dispatched()` | `true` | `false` |
| `reconcile()` | sweeps | `Ok(0)`, no query |
| W17 "every root sweeps" | true | **false** |

The row says CONFIRMED unconditionally; the guard only checks that `.reconcile()` still appears in the
three roots, which it does.

## In Scope

- Reword W17 with both halves and evidence for each
- A conditional-row convention in the header
- A sweep of the other rows for the same pattern

## Out of Scope

- Changing the sweep behaviour; teaching the guard claim semantics; the enterprise manifest

## Affected Layers

### API
- Documentation artifact only

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **Evidence:** `wiring-claims.md:85`; `can_sweep_dispatched` (port, in_memory, http); `reconcile.rs:80-86`; the three call sites

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | W17 true as written | `validate-wiring-claims.sh` |
| 2 | Evidence for both branches | review |
| 3 | Header documents the conditional form | review |
| 4 | Other rows checked for the same pattern | MR description |

## Implementation

1. Read W15-W17 and `reconcile.rs:60-90`, `in_memory.rs:99-106`, `http.rs:202-210`.
2. Reword W17; add per-branch evidence.
3. Add the header convention; sweep the other rows.

## Pitfalls

- **Do not delete the row** — the L1 half is real and the L2 half is a deliberate decline.
- **Do not widen the guard to parse prose**; a convention in the header is enough.
- **Check the other rows** — this pattern is likely not unique to W17.

## Why this is medium

The manifest is the artifact that says "this wiring was verified"; a row that is true for one backend
and false for another teaches the next reader the wrong invariant, and the guard is structurally unable
to notice.
