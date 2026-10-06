---
guardian_issue:
  id: "ISSUE-PF-REN-2"
  title: "REN: update canonical comments, tests dir, and CI script names"
  epic: "precondition-followups"
  epic_id: "EPIC-PRECONDITION-FOLLOWUPS"
  component: "ModuleRenameCode"
  module: "precondition"
  status: planned
  priority: medium
  dependencies:
    - "ISSUE-PF-REN-1"

  in_scope:
    - "Update 22 @canonical .pi/architecture/modules/consequence-gating.md comments (+anchors) in engine/src"
    - "Fix the stale 'todo!() stubs' contract-freeze note in engine/src/precondition/mod.rs"
    - "Rename engine/tests/unit/consequence-gating/ -> precondition/ and update engine/tests/unit/mod.rs module path"
    - "Rename engine/.pi/scripts/ci/check_consequence-gating_contracts.sh -> check_precondition_contracts.sh"
    - "Rename engine/.pi/scripts/ci/stage_consequence-gating_proofing.sh -> stage_precondition_proofing.sh + update stage registration"

  out_of_scope:
    - "Architecture docs (ISSUE-PF-REN-1)"
    - "Renaming code identifiers or wire strings"
    - "Relocating R2/R3 (ISSUE-PF-REL-1/2)"

  affected_layers:
    domain:
      - "Modify: canonical comments in engine/src/precondition/**"
    application:
      - "Modify: engine/src/precondition/mod.rs (doc note)"
    infrastructure:
      - "Modify: engine/.pi/scripts/ci/*"

  canonical_references:
    - module: "engine/.pi/architecture/modules/consequence-gating.md"
    - code: "engine/src/precondition/mod.rs"

  acceptance_criteria:
    - "grep -rn 'modules/consequence-gating.md' engine/src returns 0"
    - "The stale todo!() contract-freeze note is removed/reworded"
    - "engine/tests/unit/precondition/ exists; the suite reports the same test count under the new module path"
    - "CI scripts renamed; the proofing stage still runs and passes"
    - "cargo test -p rigorix-engine --lib and clippy -D warnings pass"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    Purely mechanical. Replace the canonical path token only; keep the anchor
    fragments (e.g. #dispatch-integration, #config). The test module path will
    change from consequence_gating::... to precondition::... — verify the unit
    harness picks the renamed directory. Update any stage registry that lists
    stage_consequence-gating_proofing.sh.

  file_changes:
    - "modify: engine/src/precondition/**/*.rs (canonical comments)"
    - "modify: engine/src/precondition/mod.rs"
    - "rename: engine/tests/unit/consequence-gating -> engine/tests/unit/precondition"
    - "modify: engine/tests/unit/mod.rs"
    - "rename: engine/.pi/scripts/ci/check_consequence-gating_contracts.sh -> check_precondition_contracts.sh"
    - "rename: engine/.pi/scripts/ci/stage_consequence-gating_proofing.sh -> stage_precondition_proofing.sh"
    - "modify: engine/.pi/scripts/ci/ (stage registration)"
---

# ISSUE-PF-REN-2: Source, tests, and CI

## Inventory (verified)

- 22 source files reference `modules/consequence-gating.md` (canonical comments)
- 11 test dirs under `engine/tests/unit/consequence-gating/`
- 2 CI scripts: `check_consequence-gating_contracts.sh`, `stage_consequence-gating_proofing.sh`
- stale note: `engine/src/precondition/mod.rs:52` ("method bodies are `todo!()` stubs")

## Verification

- `grep -rn 'modules/consequence-gating.md' engine/src` → 0
- `cargo test -p rigorix-engine --lib` → 2146 (unchanged)
- proofing stage still green

## References

- `engine/src/precondition/mod.rs`
- `engine/.pi/scripts/ci/check_consequence-gating_contracts.sh`
