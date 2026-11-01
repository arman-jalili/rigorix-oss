---
guardian_issue:
  id: "ISSUE-DEAD-LEGACY-REAPPROVAL-FUNCTION"
  title: "legacy_approvals_requiring_reapproval is public, documented as migration support, and called only by its own test"
  epic: "authority-boundary"
  component: "Approval"
  module: "approval"
  status: planned
  priority: low
  tracking_issue: 0
  dependencies:
    - "ADR-011 — the approval binding"

  intent: |
    `legacy_approvals_requiring_reapproval` (`approval/application/service_impl.rs:361-380`) is a
    public async function whose doc comment says it implements *"R3+AC #10 migration support —
    invalidate legacy approvals on hydrate"*. Its only caller is its own unit test
    (`engine/tests/unit/approval/approvalservice/approvalservice_test.rs:452`); no production path
    calls it, and no hydrate path uses it.

    The behaviour it documents is already provided elsewhere: `verify_intent` refuses a node with no
    record (`ApprovalError::NotFound`, "re-approval required"), which is the same outcome for a node
    carrying a legacy `approved` flag from before the binding existed. So the function is either
    redundant or a migration step that was never wired — and a reader cannot tell which from the code.

  in_scope:
    - "Delete the function and its unit test; `verify_intent`'s no-record refusal is the documented behaviour for legacy approvals"
    - "Any module-doc sentence that cites it as migration support is corrected with it"
    - "If it is *not* redundant — a hydrate path that must write records rather than refuse — then it is wired into that path and the doc says where"

  out_of_scope:
    - "The legacy `approved` set used by the dispatch gate, which is a different mechanism and is wired"
    - "The record store's own legacy-map migration, which is `ledger migrate`"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: the function is removed (or wired)"
    infrastructure:
      - "None"

  citations:
    - file: "engine/src/approval/application/service_impl.rs"
      note: ":361-380 the function and its 'R3+AC #10 migration support' doc"
    - file: "engine/src/approval/application/service_impl.rs"
      note: "verify_intent refuses a node with no record — the same end state"
    - file: "engine/tests/unit/approval/approvalservice/approvalservice_test.rs"
      note: ":452 the only caller"

  verification:
    - "`grep -rn legacy_approvals_requiring_reapproval engine/src cli mcp server actions` returns nothing"
    - "the legacy-approval scenario still refuses dispatch for a node with no record (existing seed)"
---

# ISSUE-DEAD-LEGACY-REAPPROVAL-FUNCTION

## Measured

```
$ grep -rn legacy_approvals_requiring_reapproval engine/src cli mcp server actions
engine/src/approval/application/service_impl.rs:368   pub async fn …
engine/tests/unit/approval/approvalservice/approvalservice_test.rs:452  legacy_approvals_requiring_reapproval(repo.as_ref() …
```

One definition, one test, no callers. A public function with a migration doc and no migration.

## The required end state

Gone, with the doc sentence that cites it — or wired into the hydrate path it names, and the doc says
which path. A public function that exists to be tested is a claim nobody uses.
