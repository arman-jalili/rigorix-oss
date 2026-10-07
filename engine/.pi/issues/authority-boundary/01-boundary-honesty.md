---
guardian_issue:
  id: "ISSUE-BOUNDARY-HONESTY"
  title: "Document the authority boundary honestly: operator-owned path + hook, not a sandbox"
  epic: "authority-boundary"
  component: "PreconditionTrustBoundary"
  status: planned
  priority: high
  order: 1
  dependencies: ["ADR-017 consequence gating"]
---

# ISSUE-BOUNDARY-HONESTY — say what the trust boundary is, and what it is not

## Problem

The precondition trust boundary is described as *"a check the agent can edit is
no check"*, and the demo says an agent *"with full write access to the repo
cannot forge the authority"*. Both are true **for the attack they name** — but
read together they imply **isolation**, which is not what exists.

What exists is:

1. a **path** check — `command[0]` must resolve outside the agent-writable
   workspace (`ensure_outside_workspace`, `engine/src/precondition/infrastructure/runner.rs:143`);
2. a **hook** — `.claude/hooks/deny-ledger-tamper.mjs` denies agent-mediated
   writes to the authority config.

It is **not** OS isolation. An agent with an *unmediated* write path to the
check or the authority (a different tool, a differently-privileged subprocess,
a persisted script, a compromised dependency) defeats both. And the signed
record does **not** make the tamper visible: `precondition_findings[].inputs_hash`
is the hash of the **step inputs the engine fed the check**, not the authority
the check read.

This is a *documentation* defect, not a code defect — but it is the kind that
makes a security reader lose trust when they discover it unaided.

## Scope

- **ADR-017** — extend `### Honest boundary — what this does not guarantee`
  (currently covers only the temporal `T_check`–`T_action` gap) with the
  isolation gap: the boundary is policy + path + hook, not a sandbox.
- **Module doc** `engine/.pi/architecture/modules/precondition.md` §Security
  Considerations — same caveat next to the trust-boundary row.
- **Gap ledger** — new `GAP-A-36`: *the out-of-repo boundary is a hook
  boundary, not a sandbox; inputs_hash does not cover the authority.*
- **`authority-demo`** README + ARTICLE — state the caveat where the boundary
  is claimed, and point at the isolation recipe (issue 02).

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | ADR-017 names the residual explicitly: an unmediated write path to the check/authority defeats the boundary; the guarantee is *path + hook*, not isolation |
| 2 | ADR-017 states that `inputs_hash` covers the check **inputs**, not the authority content (so a forged authority is not visible on today's record) |
| 3 | `modules/precondition.md` §Security matches the ADR, and the trust-boundary row links to it |
| 4 | `GAP-A-36` recorded with the recommended fix (issues 02–04) |
| 5 | `authority-demo` README/ARTICLE carry the caveat and link to the isolation recipe |
| 6 | No wire/behavior change |

## Files

- `engine/.pi/architecture/decisions/ADR-017-consequence-gating.md` (lines ~172–182)
- `engine/.pi/architecture/modules/precondition.md` (lines ~115, ~304, ~375)
- `engine/.pi/architecture/gap-ledger.md`
- `authority-demo/README.md`, `authority-demo/ARTICLE.md` (separate repo)

## References

- `engine/src/precondition/infrastructure/runner.rs:143` — `ensure_outside_workspace` (path containment only)
- `engine/src/precondition/domain/finding.rs:67` — finding fields (no authority binding)
- `engine/src/audit/domain/envelope.rs:343` — `PreconditionFindingRef`
- Sibling discipline: ADR-016 scopes its Phase D out explicitly rather than over-claiming
