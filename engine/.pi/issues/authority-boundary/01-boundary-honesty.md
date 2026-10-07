---
guardian_issue:
  id: "ISSUE-BOUNDARY-HONESTY"
  title: "Document the authority boundary as a per-mode strength matrix (not a snapshot claim)"
  epic: "authority-boundary"
  component: "PreconditionTrustBoundary"
  status: planned
  priority: high
  order: 1
  dependencies: ["ADR-017 consequence gating"]
---

# ISSUE-BOUNDARY-HONESTY — document boundary strength **per mode**

## Problem

Today's wording — *"a check the agent can edit is no check"*, and the demo's
*"an agent with full write access to the repo cannot forge the authority"* —
implies **isolation**. What actually exists is a **path** check plus a **hook**,
and the signed record does not bind the authority (`inputs_hash` hashes the
check *inputs*, not the authority). That is a real gap.

**But a blanket "this is only a hook, not a sandbox" would itself go stale.**
Issues #985 (isolated install), #986 (engine refuses a writable check) and #987
(signed `authority_digest`) *raise* the boundary. If #984 writes a snapshot
sentence, the docs become false the moment those land — the exact failure mode
this ADR series exists to avoid.

**Therefore: document boundary strength as a mode → guarantee matrix.** Each row
stays true in its own mode; the default row's caveat is permanent (the default
does not change); the other rows flip from *planned* to *available* as their
issue lands.

## The matrix (the deliverable)

| Mode | Enforced by | Guarantee | Does **not** cover |
|---|---|---|---|
| **default install** (today) | path containment (outside the workspace) + `PreToolUse` hook | agent-mediated tool calls cannot touch the check/authority | an **unmediated** write path to `$HOME` (other tool, differently-privileged subprocess, persisted script) |
| **`--isolated`** (#985) | OS ownership/mode (root-owned `0555` / `0444`) | the agent's **UID cannot write** the check/authority | requires no passwordless `sudo` for the agent user (setup checks this) |
| **`require_immutable_check`** (#986) | engine, at dispatch | a writable check/authority is **refused** | a check writable by a *different* privileged identity |
| **attribution** (#987) | signed envelope | a forged/changed authority is **visible** (`authority_digest`) | — (detection, not prevention) |

The **default row is the permanent caveat**: it is not a sandbox, and no future
issue changes that. The other rows are *upgrades* the operator opts into.

## Scope

- **ADR-017** `### Honest boundary — what this does not guarantee` (currently
  covers only the temporal `T_check`–`T_action` gap): add the matrix
  above and the statement that `inputs_hash` ≠ authority digest.
- **`modules/precondition.md`** §Security: the matrix (or a link), next to
  the trust-boundary row.
- **Gap ledger**: `GAP-A-36` — *default boundary is path + hook, not
  isolation; mitigated by #985–#987* (record it as mitigated-by, not "fixed", so
  the record tracks the modes).
- **`authority-demo`** README/ARTICLE: replace the flat claim with the
  matrix (or link it), and say plainly which mode the demo runs by default.

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | ADR-017 carries a **mode → guarantee** table with the four rows above (default / `--isolated` / `require_immutable_check` / attribution) |
| 2 | The **default** row states the caveat (path + hook; an unmediated write defeats it) — and is marked permanent, not "to be removed later" |
| 3 | Each non-default row cites its issue and a state (*planned* / *available*); the row is **updated**, never the caveat |
| 4 | No doc claims isolation for the default mode |
| 5 | `inputs_hash` (check inputs) vs `authority_digest` (authority content) is stated, with #987 as the remedy |
| 6 | `GAP-A-36` recorded as *mitigated by #985–#987*, not "closed" |
| 7 | `authority-demo` README/ARTICLE use the same matrix (or link it) and name the demo's active mode |
| 8 | A maintenance note: when #985/#986/#987 merge, flip their row's state; **do not** delete the default caveat |
| 9 | No wire/behavior change |

## Files

- `engine/.pi/architecture/decisions/ADR-017-consequence-gating.md` (lines ~172–182)
- `engine/.pi/architecture/modules/precondition.md` (lines ~115, ~304, ~375)
- `engine/.pi/architecture/gap-ledger.md`
- `authority-demo/README.md`, `authority-demo/ARTICLE.md` (separate repo)

## References

- `engine/src/precondition/infrastructure/runner.rs:143` — `ensure_outside_workspace` (path containment only)
- `engine/src/precondition/domain/finding.rs:67`, `engine/src/audit/domain/envelope.rs:343`
- Upgrades: #985 (isolation) · #986 (engine hardening) · #987 (attribution)
- Discipline: ADR-016 scopes its Phase D out explicitly rather than over-claiming
