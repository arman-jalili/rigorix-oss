---
guardian_issue:
  id: "ISSUE-IDP-UNREACHABLE-AT-STARTUP-DEGRADES-THE-PROCESS"
  title: "an IdP that is unreachable at MCP startup disables attestation for the life of the process, and the refusal says the caller is unauthenticated"
  epic: "authority-boundary"
  component: "Mcp"
  module: "auth"
  status: planned
  priority: high
  tracking_issue: 1277
  dependencies: []

  intent: |
    `rigorix-mcp` resolves its token verifier **once**, at startup:

    ```rust
    let verifier = match idp.discover().await {
        Ok(meta) => match meta.jwks_uri { Some(u) => JwksVerifier::new(u), None => NullVerifier },
        Err(e) => { tracing::warn!("auth: IdP discovery failed ({e}) — claims degrade to unverified");
                    Box::new(NullVerifier::new()) }
    };
    ```

    `NullVerifier` returns `IdentitySource::Unverified` and never an error. So when the IdP is not
    answering at that instant, the process keeps a verifier that cannot attest anything for as long
    as it lives — while every other auth surface keeps working. Measured on `payouts-demo` (Keycloak
    restarted, driver run immediately), three of three runs:

    ```
    startup : WARN auth: IdP discovery failed (…) — claims degrade to unverified
    login   : AuthLoginSucceeded { subject: 5404ca13-… }            ← the device flow completed
    status  : Authenticated, claim_summary: Some(ClaimSummary { … })  ← a real claim is attached
    run     : Step 'pay_a' requires an attested identity but the caller is unauthenticated
              — run rigorix_auth_login (or execute as a local principal) first
    ```

    Warm IdP: 6 of 6 runs pass. The refusal is correct as a decision (fail closed), and wrong twice
    as a report:

    1. **The caller did authenticate.** `rigorix_auth_login` completed and `rigorix_auth_status`
       returned an attested subject; the message tells the operator to do what they just did.
    2. **The cause is never named.** The condition is "this process could not verify the claim
       because the IdP was unreachable when it started", and the only recovery is restarting the MCP
       server once the IdP is up — neither appears anywhere the operator can see.

    Callers who hit this cannot distinguish it from a genuinely anonymous session, and no status
    surface distinguishes them either: `current_engine_identity` maps `Unverified` to `None`, and
    `IdentityRequired` renders that `None` as `status: "unauthenticated"`.

  in_scope:
    - "`TokenStatus` gains a fourth literal — `Unverified` / `unverified` — for \"credential material is present and cannot be checked\". The name is the word the engine already prints for this condition (`orchestrator_impl.rs:1252`), so the status output and the refusal say one word, not two"
    - "`current_engine_identity` stops collapsing `IdentitySource::Unverified` to `None`: it hands the engine the claim it has, tagged `Unverified`, so the existing refusal branch reports `unverified` instead of `unauthenticated`"
    - "A session whose attestation could not be attempted reports `unverified` in `rigorix_auth_status` and in `IdentityRequired`, and the refusal names the real recovery — restart the MCP server once the IdP answers, or fix `RIGORIX_IDP_ISSUER` — instead of `rigorix_auth_login`"
    - "Attestation does not depend on a decision made once at startup: when the verifier is the offline default because discovery failed, the next claim that needs verifying re-attempts discovery (bounded per process), so a process that outlived an IdP outage recovers without a restart"
    - "The contract notes that describe the vocabulary are updated with it: `mcp/src/auth/domain/status.rs` (the frozen note, which currently folds \"or IdP unreachable\" into `Unauthenticated`), `mcp/.pi/architecture/modules/auth.md`, `mcp/.pi/architecture/decisions/ADR-008-auth-client-flow.md`, `mcp/docs/runbook-auth.md`"
    - "Seeds: discovery failing at startup then recovering → a run succeeds without a restart; an IdP that never answers → `rigorix_auth_status` reports `unverified` and the refusal says `unverified` with the restart recovery, never `unauthenticated` for a session that logged in; the three existing literals keep their meaning and their tests"

  out_of_scope:
    - "Failing open when the IdP is unreachable; the L1 gate keeps refusing"
    - "Caching JWKS across processes"
    - "Renaming the engine's existing `unverified` refusal literal, or introducing a third spelling for the same fact"
    - "The demo-side workaround, which now waits for the discovery document before spawning the server and restarts a server that reported a discovery failure (payouts-demo `f461ca7`)"

  affected_layers:
    domain:
      - "Modified: `TokenStatus` carries `Unverified`, distinct from `Unauthenticated`"
    application:
      - "Modified: discovery is retried when attestation is needed and the verifier is the offline default"
    infrastructure:
      - "Modified: the verifier is resolved per attestation, not once per process"

  citations:
    - file: "mcp/src/host/mod.rs"
      note: ":1300-1325 discovery runs once; `Err` installs NullVerifier; :367 and :440 inject `session_identity()` per tool call"
    - file: "engine/src/identity/infrastructure/verifier.rs"
      note: ":60-80 NullVerifier returns Unverified, never an error"
    - file: "mcp/src/auth/interfaces/mcp/handler_impl.rs"
      note: ":80-95 current_engine_identity maps Unverified to None"
    - file: "engine/src/orchestrator/domain/error.rs"
      note: ":131 IdentityRequired renders that None as status 'unauthenticated'"

  verification:
    - "cold: discovery fails, IdP comes up, a run succeeds without restarting the server"
    - "cold and stays down: the refusal says the IdP is unreachable and does not say 'unauthenticated'"
    - "warm: unchanged (a verified claim is still attested and recorded)"
---

# ISSUE-IDP-UNREACHABLE-AT-STARTUP-DEGRADES-THE-PROCESS

## Measured

```
Keycloak restarted, driver run immediately (3 of 3):
  WARN  auth: IdP discovery failed … — claims degrade to unverified
  INFO  AuthLoginSucceeded { subject: 5404ca13-6ea0-4bea-97ec-b97259e954ce }
  INFO  AuthStatusChecked { status: Authenticated, claim_summary: Some(…) }
  ERR   Step 'pay_a' requires an attested identity but the caller is unauthenticated
        — run rigorix_auth_login (or execute as a local principal) first

Keycloak warm (6 of 6): all pass.
```

## Why it is worse than a cold start

The verifier is chosen before the auth service exists and is never re-chosen. A single failed
request in the first second of the process's life silently turns the L1 identity gate into "refuse
everything", for hours if the server is long-lived, while the login tool keeps reporting success.
The operator's only clue is a WARN line in a log they are not reading, and the error text points at
the one action that cannot help.

A check that cannot run must say it cannot run — and must be able to run again later.

## Decided in review

The third literal was the defect: `Unauthenticated` is documented as *"no credential material at all
(or IdP unreachable)"*, so a session that logged in without the IdP is reported as a session with no
credential. The operator extended the frozen vocabulary rather than carrying the reason beside it, so
the enum has four literals and the frozen note is rewritten to match. The literal is `unverified`,
the word the engine's refusal already prints for this exact condition — one word for one fact.
