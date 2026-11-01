---
guardian_issue:
  id: "ISSUE-UNVERIFIED-REFUSAL-NAMES-A-STARTUP-FAILURE"
  title: "the unverified refusal says the IdP was unreachable when the server started, even when it failed mid-session, and never shows the error it recorded"
  epic: "authority-boundary"
  component: "Mcp"
  module: "auth"
  status: planned
  priority: low
  tracking_issue: 1282
  dependencies:
    - "#1277 — the refusal distinguishes 'no credential' from 'could not check'"

  intent: |
    #1277 gave the refusal the right *word* and the right recovery. The parenthetical asserts a history:

    ```rust
    if status == "unverified" {
        format!("Step '{step}' requires an attested identity but the caller is unverified — the claim is \
                 present and this process could not verify it (the IdP was unreachable when the server \
                 started): restart the MCP server once the IdP answers, or fix RIGORIX_IDP_ISSUER")
    }
    ```

    Measured, with the IdP **reachable at startup** and stopped after the login completed:

    ```
    [ 0s] spawn rigorix-mcp, IdP reaches discovery; login completes; auth_status → authenticated
    [ 6s] docker compose stop
    [ 8s] auth_status → unverified
    [11s] rigorix_run → Step 'pay_a' requires an attested identity but the caller is unverified — the claim
          is present and this process could not verify it (the IdP was unreachable when the server started):
          restart the MCP server once the IdP answers, or fix RIGORIX_IDP_ISSUER
    [18s] docker compose start → auth_status → authenticated; the same run → Completed, no restart
    ```

    The IdP *was* reachable when the server started. Nothing about the process is degraded — the retry
    recovers on its own seven seconds later — and the sentence sends the operator to restart a server that
    did not need restarting, while the condition that actually held ("the last discovery attempt failed") is
    recorded and never shown:

    ```rust
    // discovery_verifier.rs:61
    last_error: Option<String>,     // set on failure, cleared on success, exposed nowhere the operator reads
    ```

  in_scope:
    - "The `unverified` refusal states the condition, not a history: the claim is present and the last identity-provider discovery attempt failed — with the recorded error when one exists"
    - "`rigorix_auth_status` reports the same reason alongside `unverified`, so the operator can see why without triggering a refusal"
    - "The recovery stays: restart once the IdP answers, or fix `RIGORIX_IDP_ISSUER` — and it does not claim a restart is required when the process has already recovered"
    - "Seeds: discovery failing after startup → the message names the failure and no startup history; discovery recovering → the status returns to `authenticated` without a restart (already true, pinned by #1277's test)"

  out_of_scope:
    - "Retrying discovery, which #1277 does"
    - "Changing the `unverified` literal"

  affected_layers:
    domain:
      - "Modified: the refusal's reason text is derived from the verifier's recorded error, not from a fixed sentence"
    application:
      - "Modified: `status()` carries the reason for `unverified`"
    infrastructure:
      - "Modified: `DiscoveryBackedVerifier::last_error` is readable by the surfaces that report it"

  citations:
    - file: "engine/src/orchestrator/domain/error.rs"
      note: ":171-177 the fixed parenthetical; :162-168 its own doc says 'unreachable at discovery'"
    - file: "mcp/src/execution_tools/domain/error.rs"
      note: ":150 the same text on the MCP side"
    - file: "mcp/src/auth/infrastructure/discovery_verifier.rs"
      note: ":61, :106, :112 last_error is recorded and cleared but never surfaced"

  verification:
    - "IdP stopped after login: the refusal names the failed discovery, not a startup failure; the status shows the same reason"
    - "IdP restarted: status returns to authenticated and the run completes without restarting the process"
---

# ISSUE-UNVERIFIED-REFUSAL-NAMES-A-STARTUP-FAILURE

## Measured

```
[ 0s] login completes, auth_status → authenticated          (IdP reachable at startup)
[ 6s] IdP stopped
[ 8s] auth_status → unverified
[11s] refusal: "…(the IdP was unreachable when the server started)…"
[18s] IdP back; auth_status → authenticated; the run completes — no restart
```

## Why it is the same defect #1277 fixed, one level down

#1277's whole point was that a refusal must describe the condition that holds rather than a plausible story
about it: `unauthenticated` was wrong for a caller who had logged in, and the advice pointed at the one action
that could not help. The new sentence gets the word and the recovery right, then asserts a startup failure
that did not happen and recommends a restart the process does not need.

The verifier already knows the real reason — it records it on every failure and clears it on success — and the
status output is where a caller would look for it.
