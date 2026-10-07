---
guardian_issue:
  id: "ISSUE-AUTHORITY-ISOLATION"
  title: "Ship an isolation recipe: setup-authority.sh --isolated (root-owned, non-writable)"
  epic: "authority-boundary"
  component: "PreconditionTrustBoundary"
  status: planned
  priority: high
  order: 2
  dependencies: ["ISSUE-BOUNDARY-HONESTY (docs)"]
---

# ISSUE-AUTHORITY-ISOLATION — make the boundary real on one machine

## Problem

The demo's boundary rests on a hook plus an out-of-repo path (see
ISSUE-BOUNDARY-HONESTY). Neither stops an unmediated write. The **real** fix
is that the agent's UID cannot write the check or the authority — an OS fact,
not a policy.

This issue ships that as a documented, scripted option so an operator can get
the strong boundary locally, without pretending the default is one.

## Design

`setup-authority.sh --isolated` installs the check and authority under a
**root-owned** path outside the repo, mode `0555` (check) / `0444`
(authority):

```
/usr/local/lib/rigorix-authority-demo/
    ├── check-beneficiary.mjs   0444 root:wheel
    └── authority.json          0444 root:wheel
```

The gate config points at that absolute path. The default (un-isolated) mode is
unchanged.

## Two traps (must be handled — this is the acceptance surface)

1. **Freeze and thaw must keep working.** Root ownership blocks the *operator's*
   own writes too — a root-owned `0444` authority cannot be edited by the
   normal user. So the operator path must escalate: `freeze_beneficiary.sh` /
   `thaw_beneficiary.sh` detect a non-writable authority and re-run the write
   via `sudo` (a documented, narrow command), or the demo documents a separate
   `authority-admin` UID. On a single-user macOS box, **sudo is the simpler
   route** and is the default the recipe documents.
2. **Passwordless sudo defeats it.** If the agent's user can run `sudo` without
   a password, root ownership protects nothing — the agent simply `sudo`s the
   write. `setup-authority.sh --isolated` MUST check `sudo -n true` for the
   invoking user and **fail closed with a clear message** (or, at minimum, warn
   loudly and record it) when passwordless sudo is available. The recipe must
   state that a no-password-sudo agent user is a **precondition** of the
   isolation claim.

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | `setup-authority.sh --isolated` installs the check (`0555`) and authority (`0444`) root-owned outside the repo and writes the gate config pointing at them |
| 2 | `freeze_beneficiary.sh` / `thaw_beneficiary.sh` still work in isolated mode (escalate via a documented sudo path) and fail with an actionable message when not permitted |
| 3 | Setup detects passwordless sudo for the invoking user (`sudo -n true`) and fails/warns, documented as a precondition of the claim |
| 4 | Default (un-isolated) mode is byte-identical to today |
| 5 | A documented check: as the agent UID, writing the check/authority fails (`EACCES`); the driver's Tₙ scene still refuses with the same signed evidence |
| 6 | README/ARTICLE state: default = policy+hook boundary; `--isolated` = OS boundary |

## Files

- `authority-demo/.rigorix/scripts/setup-authority.sh` (add `--isolated`)
- `authority-demo/.rigorix/scripts/freeze_beneficiary.sh`, `thaw_beneficiary.sh` (escalation)
- `authority-demo/reset-demo.sh` (pass-through flag)
- `authority-demo/README.md`, `ARTICLE.md`

## References

- ISSUE-BOUNDARY-HONESTY (the docs this recipe backs)
- ISSUE-TRUST-BOUNDARY-HARDENING (the engine-side detection of a writable check)
