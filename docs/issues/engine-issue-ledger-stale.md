---
guardian_issue:
  id: "ISSUE-ENGINE-ISSUE-LEDGER-STALE"
  title: "nine engine issue packets for effect-commitment still say status: planned after the epic merged, and the closed_reason convention was not applied"
  epic: "architecture readiness"
  epic_id: "EPIC-ARCHITECTURE-READINESS"
  tracking_issue: 1188
  component: "Docs"
  module: "effect-commitment"
  status: planned
  priority: low
  created_at: "2026-10-23T18:00:00Z"
  updated_at: "2026-10-23T18:00:00Z"

  intent: |
    Close the engine-side records for the shipped context, using the convention the repo already has.

    Nine packets under `engine/.pi/issues/` cover the effect-commitment components
    (`issue-effectintent`, `issue-effectstate`, `issue-effectrecord`, `issue-effectcommitmentservice`,
    `issue-effectcommitmentledger`, `issue-effectcommitmentconfig`, `issue-httpeffectledger`,
    `issue-inmemoryeffectledger`, `issue-defaulteffectcommitmentfactory`). All nine still read
    `status:planned`, although the epic merged (#1112-#1135, #1161, #1162) and the runtime gaps were
    fixed (#1163-#1166).

    The `docs/issues/` ledger was updated when those closed (`b5a231e0`); the `engine/.pi/issues/`
    ledger was not. Five other packets in the same directory already carry a `closed_reason:` field, so
    the convention exists and simply was not applied here.

    Consequence is the one this series keeps hitting: a reader (or a future `/architect` run) sees nine
    planned components for a context that ships, and cannot tell planned-for-real from stale.

  dependencies:
    - name: "engine/.pi/issues/issue-*effect*.md"
      type: internal
      note: "nine packets, all status:planned."
    - name: "closed_reason convention"
      type: internal
      note: "five packets already use it — copy the shape rather than inventing one."

  in_scope:
    - "Mark the nine packets implemented/closed with a closed_reason naming the PR or issue"
    - "Verify against code before flipping each (the packet's acceptance criteria, not just presence)"
    - "Note the convention in the module doc or a one-line header so the next epic applies it"

  out_of_scope:
    - "Other stale ledgers (a repo-wide audit is a separate question)"
    - "Deleting the packets"

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
    - evidence: "engine/.pi/issues/issue-{effectintent,effectstate,effectrecord,effectcommitmentservice,effectcommitmentledger,effectcommitmentconfig,httpeffectledger,inmemoryeffectledger,defaulteffectcommitmentfactory}.md — status:planned"
    - evidence: "five packets in the same directory carry closed_reason:"

  acceptance_criteria:
    - "All nine read implemented/closed with a reason"
    - "Each flip is justified by a code reference or the merged PR"
    - "The convention is written where the next epic will look"
    - "validate-ci.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - canonical

  implementation_notes: |
    Check each packet against the code, not against the epic's closure. A packet whose acceptance
    criteria are only partly met should say so — the point of this ledger is the difference between
    "planned" and "done", and flipping all nine in one edit reproduces the staleness in the other
    direction.

    `issue-defaulteffectcommitmentfactory` is the one whose criteria may not fully hold: the factory is
    implemented, but #1187 shows the key it stores is the placeholder. If that is in the packet's
    criteria, mark it accordingly rather than closing it.

  file_changes:
    - "modify: engine/.pi/issues/issue-effectintent.md, issue-effectstate.md, issue-effectrecord.md, issue-effectcommitmentservice.md, issue-effectcommitmentledger.md, issue-effectcommitmentconfig.md, issue-httpeffectledger.md, issue-inmemoryeffectledger.md, issue-defaulteffectcommitmentfactory.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1188: nine packets, all still planned

## What you are looking at

| packet | status | epic state |
|---|---|---|
| `issue-effectintent` … `issue-defaulteffectcommitmentfactory` (9) | `planned` | merged (#1112-#1135, #1161-#1166) |

`docs/issues/` was updated in `b5a231e0`; this ledger was not. Five peers already use
`closed_reason:`.

## In Scope

- Nine flips, each justified by code or the merged PR
- The convention noted for the next epic

## Out of Scope

- A repo-wide ledger audit; deleting packets

## Affected Layers

### API
- Documentation artifacts

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **Evidence:** the nine packets; the five peers with `closed_reason:`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Nine packets closed with reasons | `validate-canonical.sh` |
| 2 | Each justified by code/PR | review |
| 3 | Convention documented | review |

## Implementation

1. List the nine; read each packet's acceptance criteria.
2. Verify against code; flip with `closed_reason`.
3. Note the convention where the next epic starts.

## Pitfalls

- **Do not bulk-flip.** Check the criteria; `defaulteffectcommitmentfactory` is affected by #1187.
- **Do not delete the packets** — they are the record of what was specified.

## Why this is low

No behaviour is affected. But it is the same "planned means what?" ambiguity that cost a review round
on the module statuses, and the convention to fix it already exists five files away.
