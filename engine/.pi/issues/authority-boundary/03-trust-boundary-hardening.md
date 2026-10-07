---
guardian_issue:
  id: "ISSUE-TRUST-BOUNDARY-HARDENING"
  title: "Engine: detect a check writable by the agent's UID and refuse/flag it"
  epic: "authority-boundary"
  component: "PreconditionTrustBoundary"
  status: planned
  priority: high
  order: 3
  dependencies: ["ISSUE-BOUNDARY-HONESTY (docs)"]
---

# ISSUE-TRUST-BOUNDARY-HARDENING — the engine should know the boundary's strength

## Problem

`ensure_outside_workspace` (`engine/src/precondition/infrastructure/runner.rs:143`)
refuses a check that resolves **inside** the workspace. It says nothing about a
check that resolves **outside** the workspace but is still **writable by the
agent's euid** — e.g. `$HOME/.rigorix-authority-demo/check-beneficiary.mjs`,
which is exactly what the demo installs. So the engine cannot distinguish
"outside the repo" from "outside the agent's reach", and the record cannot
either.

## Design

At dispatch, after resolving `command[0]` (and any operator-declared authority
path — see ISSUE-AUTHORITY-ATTRIBUTION), assess whether it is writable by the
current euid using POSIX metadata:

- `std::os::unix::fs::MetadataExt` (uid/gid) + `PermissionsExt` (mode);
- writable if owned by euid with an owner-write bit, or group-writable and the
  euid is in the group, or world-writable.

Then:

1. **Record** the assessment (tracing + the internal finding), so the boundary
   fact is available. Promoting it into the **signed** envelope is
   ISSUE-AUTHORITY-ATTRIBUTION (a contract change) — keep the overlap explicit.
2. **Offer an opt-in refusal**: `require_immutable_check = true` on the
   precondition (or `PreconditionConfig`), additive, default `false`. When set,
   a writable check/authority is refused with a distinct outcome (e.g.
   `boundary`) — fail closed like every other precondition error.
3. Default behaviour is **unchanged**: the demo's `$HOME` install is writable
   by design today, so a hard default refusal would break it (and every current
   user). Immutability is opt-in until the isolation recipe (issue 02) is the
   documented default.

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | A writability assessment (euid vs owner/group/world mode) is computed for the resolved check; tracing reports it |
| 2 | `require_immutable_check = true` refuses a writable check/authority with a distinct, non-retryable outcome; malformed/missing metadata fails closed |
| 3 | Default (`false`) behaviour is unchanged — no regression for the `$HOME`-installed demo |
| 4 | Unit tests: writable → refused under the flag; non-writable → passes; metadata error → refused under the flag |
| 5 | Non-unix builds compile (gate the new checks behind `cfg(unix)`, best-effort elsewhere) |
| 6 | Docs: `modules/precondition.md` + ADR-017 note the strengthened boundary and the `require_immutable_check` knob |
| 7 | When this lands, flip the **`require_immutable_check`** row in the ADR-017 boundary matrix (#984) from *planned* → *available* |

## Files

- `engine/src/precondition/infrastructure/runner.rs` (assessment)
- `engine/src/precondition/domain/precondition.rs` (`require_immutable_check`, `PreconditionConfig`)
- `engine/src/precondition/domain/finding.rs` (outcome/reason)
- `engine/.pi/architecture/modules/precondition.md`, ADR-017

## References

- ISSUE-BOUNDARY-HONESTY (the residual this narrows)
- ISSUE-AUTHORITY-ISOLATION (the OS-level version of the same control)
- ISSUE-AUTHORITY-ATTRIBUTION (signs the assessment)
