# fix: #1285 — the unverified reason is recorded for a JWKS failure too, and the refusal quotes it

Implements **#1285** (`ISSUE-UNVERIFIED-REASON-IS-NULL-WHEN-DISCOVERY-SUCCEEDED`).

## The measured defect

```
[ 0s] discovery succeeds, login completes
[13s] auth_status → authenticated
[13s] the IdP is stopped (the cached JWKS verifier stays)
[15s] auth_status → {"status":"unverified","source":"unverified","reason":null,…}
[19s] refusal    → "…the last identity-provider discovery attempt failed…"
```

Three wrong things at that instant:

1. `reason` was `null` — the field exists for this answer.
2. The last discovery attempt **did not fail**; it succeeded 15 s earlier. The JWKS
   fetch inside verification is what failed.
3. "attestation re-attempts discovery" is not what recovers — the verifier is cached.
   The JWKS fetch succeeding again is (measured: `authenticated`, no rediscovery).

The mechanism was one branch:

```rust
match verifier {
    Some(verifier) => verifier.verify(token, claim).await,  // reason dropped; remember() never called
    None => Ok(Unverified { reason: …last_error… }),        // only this branch recorded
}
```

## What changes

| # | Before | After |
|---|---|---|
| 1 | JWKS failures returned their reason and recorded nothing; `last_failure()` stayed `None` | `verify` records the reason of **whatever outcome it returns**, both branches, and clears it when a credential verifies again |
| 2 | "the last identity-provider **discovery** attempt failed" | "the last **credential-verification** attempt failed: *{recorded cause}*" — no stage named |
| 3 | "attestation re-attempts **discovery**" | "verification is re-attempted on the next call" — what actually recovers with a cached verifier |
| 4 | the engine had no cause to quote | `RunFromTemplateInput`/`PlanFromTemplateInput` carry `identity_verification_failure`, filled by the MCP host beside the claim it already injects |
| 5 | `rigorix_auth_status.reason` empty for a JWKS failure | same cause as the refusal — one recorded fact, two surfaces |

### Why a request field and not `IdentityRef`

`IdentityRef` is serialized into the audit envelope, whose canonical form is mirrored
**frozen** in `rigorix-enterprise` (ADR-016). #1282 declined to put a diagnostic string
there and that decision stands. `identity_verification_failure` is request-side only —
never written to an envelope, optional, and dropped for `unauthenticated` where there is
no verification attempt to blame. The engine quotes what the verifying process recorded
instead of guessing at a stage it cannot see.

## Acceptance criteria

| # | Criterion | Status |
|---|---|---|
| 1 | Discovery succeeded, IdP then gone → `status.reason` is the JWKS failure | ✅ `a_jwks_failure_after_a_successful_discovery_is_recorded` |
| 2 | The refusal quotes the cause and never says "discovery attempt" | ✅ gate seed + DTO-hop seed + end-to-end seed |
| 3 | Recovery names re-attempted verification, not rediscovery | ✅ assertions on `verification is re-attempted on the next call` and `!contains("re-attempts discovery")` |
| 4 | IdP never reachable → unchanged from #1277/#1282 | ✅ existing verifier/gate/status tests pass unchanged |
| 5 | IdP back → `authenticated`, `reason: null` | ✅ `last_failure()` clears on a verified outcome |

## Verification

```
# the core defect, no live IdP needed: discovery answers, the JWKS port is closed
a_jwks_failure_after_a_successful_discovery_is_recorded
  ├── verifier.is_resolved() == true      (discovery succeeded, verifier cached)
  ├── idp.attempts() == 1                 (no rediscovery)
  ├── last_failure() is non-null, equals the outcome's reason, names the failing fetch
  ├── !last_failure().contains("discovery")
  └── OrchestratorError::IdentityRequired{ step, status, reason } renders the cause

unverified_refusal_quotes_the_recorded_cause    (engine gate)
test_requirement_identity_refusal_quotes_the_recorded_cause  (DTO → gate, R9 path)
unauthenticated_ignores_a_recorded_cause        (no claim, no cause)
verification_failure_hands_the_recorded_cause_to_the_engine  (MCP host seam)

cargo test --workspace                                   # 3926 passed, 0 failed
cargo clippy --workspace --all-targets -- -D warnings    # clean
cargo fmt --all --check                                  # clean
.pi/scripts/validate-{ci,tests,security,operations,integration,architecture,canonical}.sh  # all exit 0
```

Docs: `mcp/docs/runbook-auth.md` (the runbook quoted the old sentence verbatim) and the
`rigorix_auth_status` row in `mcp/.pi/architecture/modules/auth.md`.
