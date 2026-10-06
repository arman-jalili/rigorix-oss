---
guardian_issue:
  id: "EPIC-PRECONDITION-FOLLOWUPS"
  title: "Epic: precondition activation + module rename (ADR-017 follow-ups)"
  epic: "precondition-followups"
  epic_id: "EPIC-PRECONDITION-FOLLOWUPS"
  status: planned
  priority: critical
  created_at: "2026-10-05T00:00:00Z"
  updated_at: "2026-10-05T00:00:00Z"

  intent: |
    Close the two gaps left by the consequence-gating epic: (1) the ADR-017
    precondition gate is implemented and green but NEVER ATTACHED in production
    (with_precondition_gate has zero callers), so .rigorix/preconditions.toml is
    never read and no check runs end-to-end; and (2) the Guardian module is
    named consequence-gating while the implementing slice is engine/src/precondition,
    breaking the module-id/code-dir convention (7/36 module docs are already
    unmapped). Also track the optional DDD relocation of R2/R3 to their natural
    contexts.

  dependencies:
    - name: "ADR-017"
      type: internal
      note: "Design; implemented in #952-#965 (epic #937, closed)."

  in_scope:
    - "ACT: wire PreconditionSetup into ParallelExecutionFactoryConfig + composition roots"
    - "ACT: prove composition reachability with an end-to-end test"
    - "REN: rename the Guardian module consequence-gating -> precondition (docs, source, tests, CI)"
    - "REL: optional relocation of R2 -> execution_engine/dag_engine, R3 -> sequence_policy"

  out_of_scope:
    - "New precondition behavior (ADR-017 semantics are frozen)"
    - "Changing wire strings: precondition_checked, precondition_findings[], .rigorix/preconditions.toml, RIGORIX_*, precondition_denied, Precondition* identifiers"
    - "The payouts falsifier demo (tracked separately: engine/.pi/issues/issue-consequence-gating-demo.md)"
    - "SDK #39 contract fixture and enterprise #225 ingestion (separate repos)"

  canonical_references:
    - adr: "engine/.pi/architecture/decisions/ADR-017-consequence-gating.md"
    - module: "engine/.pi/architecture/modules/consequence-gating.md"
    - code: "engine/src/precondition/application/gate_impl.rs"

  acceptance_criteria:
    - "Enabling .rigorix/preconditions.toml makes the composed executor refuse a matching step whose check is non-zero"
    - "precondition_findings[] is recorded for a real composed run (not only a manual with_precondition_gate call)"
    - "The module doc is named precondition.md and zero references to modules/consequence-gating.md remain"
    - "cargo test -p rigorix-engine --lib + clippy + validate-canonical all pass after the rename"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Sequence: ACT issues first (the capability is dormant until wired), then REN
    (mechanical), then optional REL. Do NOT reopen closed epic #937 or rewrite
    merged PR titles — history stays. Keep ADR-017-consequence-gating.md as the
    umbrella decision; only the module id becomes precondition.

  file_changes:
    - "modify: engine/src/execution_engine/application/factory.rs"
    - "modify: mcp/src/host/mod.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs"
    - "modify: actions/src/main.rs"
    - "rename: engine/.pi/architecture/modules/consequence-gating.md -> precondition.md"
---

# EPIC-PRECONDITION-FOLLOWUPS

## Why

The consequence-gating epic delivered sound code (2146 lib tests, 46/46 remote
checks) but the property is **not live**: `with_precondition_gate` is never
called, so in production `precondition_gate` is `None`, `precondition_verdict`
always returns `Dispatch`, `.rigorix/preconditions.toml` is never loaded, and
`precondition_findings[]` is always empty. Separately, the module id
(`consequence-gating`) does not match its code dir (`engine/src/precondition/`).

## Issue set

| # | Issue | Theme | Priority |
|---|-------|-------|----------|
| 00 | this epic | — | — |
| 01 | `ISSUE-PF-ACT-1` factory setup | ACT | critical |
| 02 | `ISSUE-PF-ACT-2` composition roots | ACT | critical |
| 03 | `ISSUE-PF-ACT-3` reachability test | ACT | high |
| 04 | `ISSUE-PF-REN-1` architecture docs/refs | REN | medium |
| 05 | `ISSUE-PF-REN-2` source/tests/CI | REN | medium |
| 06 | `ISSUE-PF-REL-1` relocate R2 | REL | low |
| 07 | `ISSUE-PF-REL-2` relocate R3 | REL | low |

## Dependency graph

```
01 (factory) ──► 02 (composition) ──► 03 (reachability test)
        │
        └──► 04 (rename docs) ──► 05 (rename source/tests/CI)
                                     │
                                     └──► 06 (relocate R2)
                                     └──► 07 (relocate R3)
```

## Definition of Done

- ACT done: the gate is reachable from config in MCP/CLI/server compositions.
- REN done: module id `precondition`, all references updated, validators green.
- REL decided (done or explicitly deferred).
