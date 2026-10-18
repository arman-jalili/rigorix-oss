---
guardian_issue:
  id: "ISSUE-CAPACITY-DOCS-STALE-AFTER-PORT-DELETION"
  title: "docs: ADR-018 and the gap ledger still claim the deleted persistence ports exist"
  epic: "architecture hygiene"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: medium
  dependencies:
    - "#1016 / PR #1057 — deleted LlmBudgetRepository and CapacityLedgerRepository"

  intent: |
    Bring the architecture record back in line with the code after the ports were deleted.
    Two authoritative documents still describe the removed traits as live, so a reader
    following the canonical chain is told a port exists that was deliberately removed — and
    in one case the same document both lists it as open and marks it fixed.

  in_scope:
    - "ADR-018: remove the 'the LlmBudgetRepository persistence port exists but is unwired' limitation"
    - "gap-ledger A-16: remove LlmBudgetRepository from the impl-less trait list (it was deleted, not implemented)"
    - "gap-ledger A-16: resolve the self-contradiction between the Open row and the '✅ FIXED' row"
    - "Record the decision (delete, not implement) where the removal is described"
    - "Sweep for any other doc still asserting the ports exist"

  out_of_scope:
    - "Re-litigating the deletion decision (#1016 is settled)"
    - "The remaining ~9 impl-less repository traits listed by A-16 (separate work)"

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
    - adr: ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - ledger: ".pi/architecture/gap-ledger.md"
    - evidence: "engine/.pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md — limitation column still says the port 'exists but is unwired'"
    - evidence: "engine/.pi/architecture/gap-ledger.md:175 — A-16 lists LlmBudgetRepository as zero-impl and Open"
    - evidence: "engine/.pi/architecture/gap-ledger.md:235 — A-16 also recorded as '✅ FIXED'"
    - evidence: "engine/src/budget_tracking/infrastructure/mod.rs — the port is gone; only a note remains"

  acceptance_criteria:
    - "ADR-018 contains no statement that the deleted ports exist"
    - "gap-ledger A-16 no longer lists LlmBudgetRepository, and the entry is internally consistent"
    - "The deletion decision is recorded where a reader looking for the port will find it"
    - "validate-canonical.sh and validate-docs.sh pass"
    - "No doc in the repo asserts either port exists (verified by grep, not by reading)"

  validators:
    - ci
    - canonical
    - architecture

  implementation_notes: |
    - The code already documents the removal in the two infrastructure/mod.rs files; the
      architecture record is the part that drifted.
    - gap-ledger A-16 mixes two statements about the same row. Decide one status and make the
      other agree.
    - Keep the history: say 'removed (#1016)', not 'never existed'. The distinction matters
      for anyone reading the ADR as a design record.

  file_changes:
    - "modify: engine/.pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - "modify: engine/.pi/architecture/gap-ledger.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-DOCS-STALE-AFTER-PORT-DELETION

## The drift

`#1016` / PR `#1057` deleted `LlmBudgetRepository` and `CapacityLedgerRepository`. The code was
updated (both `infrastructure/mod.rs` files explain the removal). Two architecture documents
were not.

### 1. ADR-018 still asserts the port exists

```text
| budget_tracking — LlmBudget + RAII LlmBudgetReservation | ... | the LlmBudgetRepository
  persistence port exists but is **unwired** |
```

It does not exist. It was removed as dead code.

### 2. The gap ledger contradicts itself

| line | says |
|---|---|
| `gap-ledger.md:175` | A-16 lists `LlmBudgetRepository` among "~10 impl-less repository traits", status **⬜ Open** |
| `gap-ledger.md:235` | A-16 → "✅ **FIXED** — All 9 checked traits now have 1 impl each" |

Both cannot be true, and neither matches what happened: the trait was **deleted**, not
implemented.

## Why it matters

`ADR-018` is the canonical design record for the capacity ledger and the reason the two
mechanisms exist. A reader following the canonical chain is told a persistence port is present
and merely unwired, which invites them to wire it — the exact opposite of the decision.

The gap ledger is the list of known gaps. An entry that says both "open" and "fixed" is
unusable for planning.

## Acceptance Criteria

See front matter. Verified by grep: no doc asserts either port exists.
