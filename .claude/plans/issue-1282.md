# Issue #1282 — the unverified refusal names a startup failure that may not have happened

**Branch:** `issue/1282` · **Epic:** authority-boundary · **Component:** Mcp · **Module:** auth
**Priority:** low · **Dependencies:** #1277 (merged)

## Defect

#1282 is the same defect #1277 fixed, one level down: the verdict is right, the *story* is
invented. The parenthetical asserts the IdP "was unreachable when the server started" — measured
with a healthy startup and the IdP stopped afterwards — and recommends a restart the process does
not need (the retry recovers on its own). The verifier already records the real reason on every
failure and clears it on success; nothing reads it.

## What changes

| surface | before | after |
|---|---|---|
| `IdentityRequired` (engine + MCP mirror) | "…could not verify it (the IdP was unreachable when the server started): restart the MCP server…" | "…the last identity-provider discovery attempt failed: re-run once the IdP answers (attestation re-attempts discovery), or fix RIGORIX_IDP_ISSUER; rigorix_auth_status reports the recorded error" |
| `rigorix_auth_status` | `{status, claim_summary, source}` | `+ reason?` — the verifier's recorded failure, present only beside `unverified` |
| `DiscoveryBackedVerifier` | `last_error` recorded, surfaced nowhere | `TokenVerifier::last_failure()` (sync) + `IdentityAttestationService::verification_failure()` (default `None`) |
| degraded claim reason | "restart the MCP server once the IdP answers" | "re-attempts discovery on the next call, so this clears once the IdP answers" |

Both trait methods are **defaulted**, so `NullVerifier`/`JwksVerifier` and every existing
implementor keep compiling unchanged.

## The one thing not threaded, and why

The refusal does not embed the free-form error. It could only reach it through `IdentityRef`,
which is serialized into the audit envelope — and that canonical form is mirrored frozen in
the enterprise repository (ADR-016). Adding a diagnostic string there would change the signed bytes of
every envelope for a message. The refusal states the condition and points at
`rigorix_auth_status`, which carries the recorded error; the two surfaces report the same facts.

## Verification

- engine gate: the unverified refusal contains "last identity-provider discovery attempt failed",
  contains `RIGORIX_IDP_ISSUER` and `rigorix_auth_status`, and contains **neither** "when the
  server started" nor "restart" nor `rigorix_auth_login`.
- verifier: after a failed discovery the reason names the cause and no restart; `last_failure()`
  returns the underlying error; both clear when discovery succeeds.
- status: `unverified` carries `reason` ("connection refused"), `authenticated` carries none even
  when an older attempt failed; `rigorix_auth_status` JSON includes it.
- workspace: 3921 tests, clippy/fmt clean, all validators PASS.
