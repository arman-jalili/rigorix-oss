---
guardian_issue:
  id: "ISSUE-INTERPRETER-CHECK-BOUNDARY"
  title: "Interpreter-based checks bypass the trust boundary and mis-bind attribution (command[0] only)"
  epic: "authority-boundary"
  component: "PreconditionTrustBoundary"
  status: planned
  priority: critical
  dependencies: ["ADR-017 (#985/#986/#987 landed)"]
---

# ISSUE-INTERPRETER-CHECK-BOUNDARY — argv[0] is not the check

## Problem

Everything the engine guarantees about a precondition's **integrity** is
computed from `command[0]`:

| Guarantee | Site |
|---|---|
| trust boundary (`command[0]` resolves outside the workspace) | `runner.rs:327` → `ensure_outside_workspace(&resolved, …)` |
| `check_digest` (sha256 of the check) | `runner.rs:344` → `sha256_file(&resolved)` |
| `check_writable` (boundary strength) | `runner.rs:328` → `assess_boundary(precondition, &resolved, "check")` |

where `resolved = resolve_program(command.first())` (`runner.rs:134`).

So when `command[0]` is an **interpreter**, the guarantee attaches to the
interpreter and the actual check is invisible to all three:

```toml
[[preconditions]]
id = "beneficiary-authorized"
command = ["node", "/path/to/check-beneficiary.mjs"]   # argv[0] is `node`
```

- **Trust boundary**: `node` resolves to `/opt/homebrew/bin/node` — *outside*
  the workspace — so the boundary **passes**. The script in `argv[1]` is never
  validated. An agent that can write that script rewrites the check to
  `process.exit(0)` and the gate opens.
- **`check_digest`** hashes **node**, not the check. It does not bind the
  artifact the operator authored, so "a changed check changes the digest" is
  false for this shape — the record cannot detect a rewritten check.
- **`check_writable`** reports node's writability (`false`, root- or
  brew-owned), so the signed record *asserts an immutable boundary* while the
  writable script sits in `argv[1]`. This is the worst of the three: the record
  is not merely incomplete, it is **misleading** — it claims a boundary the
  operator does not have.

This was found while validating the `authority-demo`: its check was invoked as
`["node", "$HOME/.rigorix-authority-demo/check-beneficiary.mjs"]`, and the
signed finding showed `check_writable: false` with a `check_digest` that did
not match the check. The demo now invokes the script directly (`command =
["…/check-beneficiary.mjs"]`, shebang), which routes all three back through
`command[0]` — but that is a workaround, not a fix: the engine still accepts the
interpreter shape silently.

## Why it matters

ADR-017's core claim is *"a check the agent can edit is no check"*. For any
check written as `interpreter + script` — the natural shape for a Node/Python/shell
check, and what the demos and most operators would write first — the claim does
not hold, and #987's attribution actively misreports the boundary as immutable.

## Reproduction

```toml
# .rigorix/preconditions.toml
[[preconditions]]
id = "gate"
command = ["node", ".rigorix/check.mjs"]   # script INSIDE the agent-writable workspace
```

1. The run is **not** refused with `PreconditionError::TrustBoundary` — `node`
   is outside the workspace, so the boundary passes.
2. `.rigorix/check.mjs` (agent-writable) runs and decides the verdict.
3. The finding's `check_digest` is the digest of `node`; `check_writable` is
   `node`'s writability.

## Fix options (recommend 1, or 1+3)

1. **Resolve the real artifact.** Apply the trust boundary, the digest, and the
   writability assessment to **every argv element that resolves to an existing
   regular file** (typically `argv[1]` for an interpreter invocation), not just
   `argv[0]`. Refuse when any of them resolves inside the workspace.
2. **Declare it.** Add a `check_path` alongside the existing `authority_path`:
   the operator names the check artifact explicitly, and the engine validates +
   digests that. Explicit, but pushes the burden onto the operator.
3. **Fail closed on the ambiguous shape.** If `command.len() > 1` and `argv[1]`
   resolves to a file inside the workspace, refuse with `TrustBoundary`
   regardless of `argv[0]`. Cheapest targeted closure of the bypass.

Any option must keep the wire/evidence additive: `check_digest` keeps its name
and meaning ("the check artifact"), and pre-attribution envelopes stay valid.

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | `command = ["node", "<workspace>/check.mjs"]` is refused by the trust boundary (or the script is validated + digested), with a test proving it |
| 2 | `check_digest` binds the actual check artifact, not the interpreter — a rewritten check changes the digest |
| 3 | `check_writable` reflects the check artifact's writability; it must not report `false` while the real check is agent-writable |
| 4 | A check invoked via interpreter **outside** the workspace still works unchanged |
| 5 | `require_immutable_check = true` refuses the interpreter shape when the script is writable |
| 6 | Docs: ADR-017 §Honest boundary + `modules/precondition.md` state that the boundary/digests cover the check artifact (however invoked), and note the recommended direct-invocation form |
| 7 | No wire change; absent `check_digest`/`check_writable` stays "pre-attribution", not "tampered" |

## Files

- `engine/src/precondition/infrastructure/runner.rs` (`resolve_program`, `ensure_outside_workspace`, `assess_boundary`, `sha256_file`)
- `engine/src/precondition/domain/precondition.rs` (docs / optional `check_path`)
- `engine/.pi/architecture/decisions/ADR-017-consequence-gating.md` (§Honest boundary)
- `engine/.pi/architecture/modules/precondition.md`

## References

- ADR-017 §Honest boundary (the mode → guarantee matrix)
- `engine/src/precondition/infrastructure/runner.rs:134,327,328,344`
- Found validating `authority-demo` (its `setup-authority.sh` now invokes the check directly — the workaround this issue replaces)
- Related: #985/#986/#987 (boundary modes + attribution)
