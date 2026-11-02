---
guardian_issue:
  id: "ISSUE-UNVERIFIED-REASON-IS-NULL-WHEN-DISCOVERY-SUCCEEDED"
  title: "an unverified session reports reason null when discovery had succeeded, and the refusal says the discovery attempt failed"
  epic: "authority-boundary"
  component: "Mcp"
  module: "auth"
  status: planned
  priority: medium
  tracking_issue: 1285
  dependencies:
    - "#1282 — the status carries the cause and the refusal names the condition"

  intent: |
    #1282 added `reason` to `rigorix_auth_status` and replaced the refusal's invented startup history with
    *"the last identity-provider discovery attempt failed"*. Measured on the case that happens most often — a
    session that was working, whose IdP then goes away:

    ```
    [ 0s] rigorix-mcp starts, discovery succeeds, login completes
    [13s] auth_status → authenticated
    [13s] docker compose stop   (the IdP is gone; the process has a cached JWKS verifier)
    [15s] auth_status → {"status":"unverified","source":"unverified","reason":null,"claim_summary":{…}}
    [17s] auth_status → {"status":"unverified","source":"unverified","reason":null,…}
    [19s] rigorix_run → "…the caller is unverified — the claim is present and the last identity-provider
          discovery attempt failed: re-run once the IdP answers (attestation re-attempts discovery), or
          fix RIGORIX_IDP_ISSUER"
    ```

    Three facts, all wrong for this instant:

    1. **`reason` is `null`.** The field exists for this answer and is empty exactly when the operator needs it.
    2. **The last discovery attempt did not fail** — it succeeded. What failed was the JWKS fetch inside
       verification.
    3. **"attestation re-attempts discovery"** is not what recovers here: the verifier is cached, so recovery
       comes from the JWKS fetch succeeding again (measured: `authenticated` seven seconds later, no rediscovery).

    The mechanism is one branch:

    ```rust
    match verifier {
        Some(verifier) => verifier.verify(token, claim).await,   // JwksVerifier's Unverified reason is dropped:
                                                                 // `remember()` is never called
        None => Ok(VerificationOutcome::Unverified { reason: …last_error… }),
    }
    ```

    and `last_failure()` reads only the slot that discovery failures write (`remember()`), which a successful
    discovery clears. So the outcome carries a reason (`JwksVerifier` produces one) and the surface reads a
    different, empty place.

  in_scope:
    - "`DiscoveryBackedVerifier::verify` records the reason whenever the outcome it returns is `Unverified` — both branches — so `last_failure()` and `status.reason` carry the cause for a JWKS failure as they do for a discovery failure"
    - "The refusal names the condition without choosing a stage it cannot know: the claim is present and the last credential-verification attempt failed, quoting the recorded reason"
    - "The recovery sentence follows what actually recovers: re-run once the IdP answers (verification is re-attempted on the next call), or fix `RIGORIX_IDP_ISSUER` — and does not promise a rediscovery the cached verifier will not perform"
    - "Seeds: discovery succeeded then the IdP disappears → `reason` is non-null and the refusal quotes it, never the words 'discovery attempt'; discovery never succeeded → unchanged from #1277/#1282; recovery → `authenticated` with `reason: null`"

  out_of_scope:
    - "Changing the cached-verifier design so a JWKS failure triggers rediscovery; the JWKS fetch is what recovers"
    - "Embedding a free-form error in `IdentityRef`, which #1282 deliberately declined because of the signed envelope bytes"

  affected_layers:
    domain:
      - "Modified: the `Unverified` outcome's reason is what the refusal reports, whichever stage produced it"
    application:
      - "Modified: `status()` reports the reason it now receives in both cases"
    infrastructure:
      - "Modified: `DiscoveryBackedVerifier::verify` remembers the reason it returns"

  citations:
    - file: "mcp/src/auth/infrastructure/discovery_verifier.rs"
      note: ":152-171 the two branches; :172-174 last_failure reads only the discovery slot"
    - file: "mcp/src/auth/application/service_impl.rs"
      note: ":417-424 reason = attestation.verification_failure()"
    - file: "engine/src/orchestrator/domain/error.rs"
      note: "the refusal's 'last identity-provider discovery attempt failed'"

  verification:
    - "IdP stopped after a successful discovery: status.reason is the JWKS failure, the refusal quotes it"
    - "IdP never reachable: unchanged (#1277/#1282 seeds keep passing)"
    - "IdP back: status authenticated, reason null"
---

# ISSUE-UNVERIFIED-REASON-IS-NULL-WHEN-DISCOVERY-SUCCEEDED

## Measured

```
auth_status → {"status":"unverified","source":"unverified","reason":null,"claim_summary":{…}}
refusal     → "…the last identity-provider discovery attempt failed: re-run once the IdP answers
                (attestation re-attempts discovery)…"
```

with discovery having succeeded 15 seconds earlier, and the JWKS fetch being what actually failed.

## Why it is the same rule again

#1277 was "do not say unauthenticated about a caller who logged in". #1282 was "do not say the IdP was
unreachable at startup when it failed later". This is the third layer of the same sentence: the status
has a `reason` field that is empty, and the refusal names the stage that did not fail. The outcome the
verifier already produced carries the truth; the two surfaces that report it must read it from there.
