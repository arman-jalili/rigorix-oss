# Batch 4 — Issue #1013

**Branch:** `issue/1013`
**Scope:** README-only positioning fix (Part 2 / CHANGELOG was already delivered).

## Defect
`README.md` said "See the comparison note below" but no comparison section
existed — a dangling cross-reference shipped by #1014.

## Change
- Replace the dangling line with a real `## Where Rigorix fits` section.
- Compare on the six credential-binding failure modes (scope, argument,
  temporal, tool, delegation, semantic), each mapped to an existing control and
  ADR — not to named competitors (avoids the category error).
- Add "What this table is not": no benchmark, no competitor ranking, no
  independent validation; L1/L2/L3 claim ceiling applies; unsourced competitor
  cells deliberately omitted.
- Keep release history in `CHANGELOG.md` (already done); README links to it.

## Acceptance
- No dangling cross-reference remains (both anchors verified).
- No coding assistant listed as a competitor.
- Every Rigorix advantage traceable to an ADR/spec.
- Table states what it does not establish.
- `validate-docs.sh` passes (links + changelog + identifiers).
