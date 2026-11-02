# fix: #1282 — the unverified refusal states the condition, and the status carries the cause

Implements **#1282** (`ISSUE-UNVERIFIED-REFUSAL-NAMES-A-STARTUP-FAILURE`).

## What was wrong

#1277 gave the refusal the right *word* and the right *recovery*, then asserted a history:

```rust
format!("… the caller is unverified — the claim is present and this process could not verify it
         (the IdP was unreachable when the server started): restart the MCP server once the IdP
         answers, or fix RIGORIX_IDP_ISSUER")
```

Measured with the IdP **reachable at startup** and stopped after login:

```
[ 0s] IdP reaches discovery; login completes; auth_status → authenticated
[ 6s] IdP stopped
[ 8s] auth_status → unverified
[11s] refusal: "…(the IdP was unreachable when the server started)…"
[18s] IdP back; auth_status → authenticated; the run completes — no restart
```

The IdP *was* reachable when the server started, nothing about the process is degraded, and the
retry recovers on its own seven seconds later. Meanwhile the condition that actually held — "the
last discovery attempt failed" — was recorded on every failure and surfaced nowhere:

```rust
// discovery_verifier.rs:61
last_error: Option<String>,     // set on failure, cleared on success, exposed nowhere the operator reads
```

## What changes

| surface | before | after |
|---|---|---|
| `IdentityRequired` (engine + MCP mirror) | "…could not verify it (the IdP was unreachable when the server started): restart the MCP server…" | "…the claim is present and the last identity-provider discovery attempt failed: re-run once the IdP answers (attestation re-attempts discovery), or fix `RIGORIX_IDP_ISSUER`; `rigorix_auth_status` reports the recorded error" |
| `rigorix_auth_status` | `{status, claim_summary, source}` | `+ reason?` — present only beside `unverified`, carrying the verifier's recorded failure |
| `DiscoveryBackedVerifier` | recorded the reason, surfaced it nowhere | `TokenVerifier::last_failure()` (sync-readable) |
| degraded claim's reason | "restart the MCP server once the IdP answers" | "re-attempts discovery on the next call, so this clears once the IdP answers" |

`TokenVerifier::last_failure()` and `IdentityAttestationService::verification_failure()` are
**defaulted**, so `NullVerifier`, `JwksVerifier` and every existing implementor compile unchanged;
only the discovering verifier has something to say.

## Why the refusal does not embed the error text

It could only reach it through `IdentityRef`, which is serialized into the audit envelope — and
that canonical form is mirrored **frozen** in `rigorix-enterprise` (ADR-016). A diagnostic string
is not worth changing the signed bytes of every envelope. So the refusal states the condition and
points at the surface that has the cause; `rigorix_auth_status` reports the same facts beside the
same word.

## Acceptance criteria

| # | Criterion | Status |
|---|-----------|--------|
| 1 | Refusal states the condition, not a history; recorded error surfaced when one exists | ✅ text has no "when the server started" and no "restart"; `rigorix_auth_status.reason` carries it |
| 2 | `rigorix_auth_status` reports the same reason beside `unverified` | ✅ `StatusOutput.reason`, `Authenticated` always `None` |
| 3 | Recovery stays, without claiming a restart is required | ✅ "re-run once the IdP answers (attestation re-attempts discovery)" |
| 4 | Recovery returns to `authenticated` without a restart | ✅ pinned by #1277's `discovery_is_re_attempted_after_the_cooldown` |

## Verification

```
engine gate refusal (unverified):
  contains "last identity-provider discovery attempt failed", "RIGORIX_IDP_ISSUER",
           "rigorix_auth_status"
  lacks    "when the server started", "restart", "rigorix_auth_login"

verifier:   after a failed discovery → reason names the cause, no restart claim,
            last_failure() = "the IdP was unreachable (connection refused)"
            after recovery      → last_failure() = None
status:     unverified → reason contains "connection refused", no invented history
            authenticated → reason None even when an older attempt failed
            handle_auth_status JSON carries "reason" beside "unverified"

cargo test --workspace                    # 3921 passed, 0 failed
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo fmt --all --check                   # clean
.pi/scripts/validate-{ci,tests,security,operations,integration,architecture,canonical}.sh  # all exit 0
```

Docs updated: `mcp/docs/runbook-auth.md` (the runbook quoted the old sentence verbatim) and the
`rigorix_auth_status` row in `mcp/.pi/architecture/modules/auth.md`.
