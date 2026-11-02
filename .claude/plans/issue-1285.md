# Issue #1285 — the unverified reason is empty exactly when the operator needs it

**Branch:** `issue/1285` · **Epic:** authority-boundary · **Component:** Mcp · **Module:** auth
**Priority:** medium · **Depends on:** #1282 (merged)

## Defect

Measured on the case that happens most often — a working session whose IdP goes away:

```
[ 0s] discovery succeeds, login completes
[13s] auth_status → authenticated
[13s] the IdP is stopped (the cached JWKS verifier stays)
[15s] auth_status → {"status":"unverified","source":"unverified","reason":null,…}
[19s] refusal    → "…the last identity-provider discovery attempt failed…"
```

Three wrong things at that instant:

1. `reason` is `null` — the field exists for this answer.
2. The last discovery attempt **did not fail**; it succeeded 15 s earlier. The JWKS
   fetch inside verification is what failed.
3. "attestation re-attempts discovery" is not what recovers — the verifier is cached;
   the JWKS fetch succeeding again is what recovers (measured: `authenticated`, no
   rediscovery).

The mechanism is one branch in `DiscoveryBackedVerifier::verify`:

```rust
match verifier {
    Some(verifier) => verifier.verify(token, claim).await,  // reason dropped; remember() never called
    None => Ok(Unverified { reason: …last_error… }),        // only this branch records
}
```

and `last_failure()` reads only the slot `remember()` writes, which a successful
discovery clears. The outcome already carries the truth; the surface reads elsewhere.

## What changes

| # | Change | File |
|---|---|---|
| 1 | `verify` records the reason of **whatever outcome it returns**, both branches; a `Verified` outcome clears it | `mcp/src/auth/infrastructure/discovery_verifier.rs` |
| 2 | The refusal stops naming a stage: "the last **credential-verification** attempt failed: {recorded cause}" | `engine/src/orchestrator/domain/error.rs` + MCP mirror |
| 3 | The recovery sentence follows what actually recovers: "verification is re-attempted on the next call" (no rediscovery promise) | same |
| 4 | The recorded cause reaches the refusal: `RunFromTemplateInput`/`PlanFromTemplateInput` gain `identity_verification_failure`, filled by the process that verified | engine DTOs + MCP host → facade |
| 5 | `rigorix_auth_status.reason` is non-null for a JWKS failure too | falls out of 1 |

### Why a request field and not `IdentityRef`

`IdentityRef` is serialized into the audit envelope, whose canonical form is mirrored
**frozen** in `rigorix-enterprise` (ADR-016) — #1282 declined to put a diagnostic string
there and that decision stands. `identity_verification_failure` is request-side only: it
is never written to an envelope, it is the observation the calling process made about its
own verifier, and the engine only quotes it. Optional, so a caller without a verifier
(engine/server/CLI direct) still refuses with a true, cause-free sentence.

## Acceptance criteria

| # | Criterion | How it is checked |
|---|---|---|
| 1 | Discovery succeeded, IdP then gone → `status.reason` is the JWKS failure | verifier test: cached verifier, failing JWKS → `last_failure()` is non-null |
| 2 | The refusal quotes the cause and never says "discovery attempt" | gate test: reason passed → message contains the cause, not "discovery attempt" |
| 3 | Recovery sentence names re-attempted verification, not rediscovery | message assertions |
| 4 | IdP never reachable → unchanged from #1277/#1282 | existing tests keep passing |
| 5 | IdP back → `authenticated`, `reason: null` | existing test: `last_failure()` clears |

## Verification

- `cargo test --workspace` (no regressions), clippy `-D warnings`, `fmt --check`
- `.pi/scripts/validate-*.sh` all exit 0
- Seeded proof: a fake IdP whose JWKS fetch fails after a successful discovery, driven
  through `DiscoveryBackedVerifier` (no live IdP needed for the unit-level seed).
