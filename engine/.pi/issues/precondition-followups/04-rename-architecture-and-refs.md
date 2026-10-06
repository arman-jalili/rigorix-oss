---
guardian_issue:
  id: "ISSUE-PF-REN-1"
  title: "REN: rename module doc to precondition.md + architecture references"
  epic: "precondition-followups"
  epic_id: "EPIC-PRECONDITION-FOLLOWUPS"
  component: "ModuleRenameDocs"
  module: "precondition"
  status: planned
  priority: medium
  dependencies:
    - "EPIC-PRECONDITION-FOLLOWUPS"

  in_scope:
    - "Rename engine/.pi/architecture/modules/consequence-gating.md -> precondition.md; retitle to 'Precondition Architecture'"
    - "Update the module doc header/invocation note"
    - "Update engine/.pi/architecture/CHANGELOG.md references"
    - "Update engine/.pi/architecture/gap-ledger.md (GAP-A-31..34 module-doc refs)"
    - "Update ADR-017 'Module doc' reference to precondition.md (filename stays consequence-gating as the umbrella)"
    - "Rename engine/docs/runbook-consequence-gating.md + dr-plan-consequence-gating.md -> *-precondition.md and update refs"

  out_of_scope:
    - "Source canonical comments (ISSUE-PF-REN-2)"
    - "Tests/CI (ISSUE-PF-REN-2)"
    - "Renaming the ADR file (kept as the umbrella)"

  affected_layers:
    docs:
      - "Rename: engine/.pi/architecture/modules/consequence-gating.md"
      - "Modify: engine/.pi/architecture/CHANGELOG.md"
      - "Modify: engine/.pi/architecture/gap-ledger.md"
      - "Modify: engine/.pi/architecture/decisions/ADR-017-consequence-gating.md"

  canonical_references:
    - adr: "engine/.pi/architecture/decisions/ADR-017-consequence-gating.md"
    - pattern: "engine/.pi/architecture/modules/planning-pipeline.md (concept-named module doc precedent)"

  acceptance_criteria:
    - "Module doc exists as engine/.pi/architecture/modules/precondition.md"
    - "No reference to modules/consequence-gating.md remains in engine/.pi or engine/docs"
    - "ADR-017 filename unchanged; its Module-doc reference points to precondition.md"
    - "Doc-only change: no wire string or code identifier renamed"

  validators:
    - ci
    - architecture
    - canonical

  implementation_notes: |
    ADR-017 is the umbrella decision; keep its filename. The module is the
    bounded context; name it precondition so it matches engine/src/precondition.
    This mirrors existing concept-named module docs (planning-pipeline ->
    src/planning, tool-system -> src/tools). Do not touch wire strings.

  file_changes:
    - "rename: engine/.pi/architecture/modules/consequence-gating.md -> engine/.pi/architecture/modules/precondition.md"
    - "rename: engine/docs/runbook-consequence-gating.md -> engine/docs/runbook-precondition.md"
    - "rename: engine/docs/dr-plan-consequence-gating.md -> engine/docs/dr-plan-precondition.md"
    - "modify: engine/.pi/architecture/CHANGELOG.md"
    - "modify: engine/.pi/architecture/gap-ledger.md"
    - "modify: engine/.pi/architecture/decisions/ADR-017-consequence-gating.md"
---

# ISSUE-PF-REN-1: Rename module doc + architecture refs

## Why

Guardian's module id is `consequence-gating`, but the bounded context / code dir
is `engine/src/precondition/`. Renaming the doc aligns the module id with the
code and completes the module→implementation mapping.

## Inventory (verified)

- module doc: `engine/.pi/architecture/modules/consequence-gating.md` (1)
- docs: `engine/docs/runbook-consequence-gating.md`, `engine/docs/dr-plan-consequence-gating.md` (2)
- CHANGELOG + gap-ledger + ADR-017 refs

## Must NOT change

`precondition_checked`, `precondition_findings[]`, `.rigorix/preconditions.toml`,
`RIGORIX_*`, `precondition_denied`, and all `Precondition*` code identifiers.

## References

- `engine/.pi/architecture/modules/planning-pipeline.md` (precedent: concept-named module)
- ADR-017
