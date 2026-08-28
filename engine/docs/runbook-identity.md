# Identity Module Runbook

> **Last updated:** 2026-08-28
> **Module:** Identity (`engine/src/identity/`)
> **Components:** IdentityClaim, IdentitySource, IdentityAttestationService, TokenVerifier (NullVerifier/JwksVerifier), IdentityRepository, IdentityError

## Overview

The identity module makes the human behind a run or an approval a **first-class, recorded fact**. It attests a presented IdP token (or local principal) into a structured, time-bound `IdentityClaim`, best-effort verifies it against the IdP JWKS when online, and flows a redacted summary into the audit envelope's `identity` block.

**The seam (ADR-012):** OSS attests; Enterprise authorizes. This module makes **no authorization judgment** — it records who was presented as acting and marks the verification outcome.

## Startup Sequence

1. **Module registration** happens at crate compile time — `src/lib.rs` exports `pub mod identity;`. No runtime initialization is required for the domain types.
2. **Attestation service** is constructed on demand:
   - `IdentityAttestationServiceImpl::new()` — offline default (`NullVerifier`): attestation proceeds without network verification; claims degrade to `IdentitySource::Unverified` explicitly
   - `IdentityAttestationServiceImpl::with_verifier(Box<dyn TokenVerifier>)` — JWKS-backed: pass a `JwksVerifier::new(idp_jwks_url)` for online best-effort verification
3. **IdentityRepository** (`FileSystemIdentityRepository::new(state_dir)`) is created when durable identity records are needed; it shares the state-persistence directory convention and creates it if absent.
4. **Consumption** — the orchestrator's `RunInput.identity` (an `IdentityClaim`) is converted via `IdentityRef::from_claim` (redaction) into the audit envelope's `identity` block.

## Dependencies

| Dependency | Required | Source |
|-----------|----------|--------|
| `serde` / `serde_json` | Yes | Claim/ref serialization, JWT payload parsing |
| `chrono` | Yes | Claim lifetime (`issued_at`, `expires_at`) |
| `async-trait` | Yes | Trait-object-safe async service/verifier/repository |
| `base64` | Yes | JWT header/payload/signature base64url decode |
| `rsa` | Yes (online) | RS256 signature verification (JwksVerifier) |
| `reqwest` | Yes (online) | JWKS endpoint fetch (5s timeout) |
| `tokio::fs` | Yes (persistence) | Identity record read/write (atomic write-rename) |
| `uuid` | Yes | Execution ID keys for repository records |
| `tracing` | Yes | Instrumented spans (SpanPrivacy: raw tokens skipped) |

## Graceful Shutdown

The identity module is **stateless between calls** — no long-lived connections, no background tasks, no queues.

1. **Stop attesting** — no new `attest()` calls (callers gate on their own lifecycle).
2. **Flush pending verification** — in-flight `JwksVerifier::verify` HTTP requests complete within the 5s client timeout; no unbounded waits.
3. **Durability** — identity records are written atomically (`{id}.identity.json.tmp` → `{id}.identity.json`); a crash during write leaves the previous record intact.

```bash
# Shutdown sequence (conceptual)
# No explicit teardown required — drop the service/repository handles.
# In-flight reqwest JWKS requests bounded by 5s timeout.
```

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| IdP unreachable | `VerificationOutcome::Unverified { reason }`; claim source degrades to `IdentitySource::Unverified` | **No action required** — by design (ADR-012 fail-open with explicit marker). Runs/approvals continue; the envelope marks `identity: unverified`. Never block a local dev tool because the IdP is down. |
| Tampered token | Signature mismatch → `Unverified`; claim authority cleared | Reject the presented identity at the approval/rbac boundary (Enterprise). The claim is honestly marked unverified — off-band verification possible via `token_ref`. |
| Expired token | `IdentityError::Expired` from `extract_claims` | Re-authenticate (re-run the device flow / re-present a fresh token). Not retriable. |
| Malformed token | `IdentityError::InvalidToken` | Reject the presented identity — contract: not retriable. |
| Unknown JWKS `kid` | `Unverified { reason: "no JWKS key for kid" }` | Rotate the IdP signing key or update the verifier's JWKS URL; wait for JWKS cache refresh. |
| JWKS endpoint slow/down | `Unverified` (5s timeout) | Check IdP availability; verify is best-effort — attestation continues degraded. |
| Identity record corrupt | `IdentityError::Internal("deserialize claim")` on `load` | Remove the corrupt `{id}.identity.json`; the execution's identity falls back to absent (`None`) — rerun attestation if evidence is required. |
| State directory unwritable | `IdentityError::Internal` on `save` | Fix directory permissions; the atomic pattern guarantees the previous record survives failed writes. |

## Configuration Reference

| Setting | Type | Default | Purpose |
|---------|------|---------|---------|
| IdP JWKS URL | `String` | — (offline `NullVerifier`) | Endpoint for `JwksVerifier` (e.g. `https://idp.example.com/.well-known/jwks.json`) |
| JWKS fetch timeout | `Duration` | 5s | Bounds network waits during best-effort verification |
| Identity state dir | `PathBuf` | — | Directory for `{execution_id}.identity.json` records (shares state-persistence dir) |
| Verification toggle | `TokenVerifier` choice | `NullVerifier` | `NullVerifier` (offline) or `JwksVerifier` (online) |

## Verification Workflow

1. Human logs in via the MCP auth module (OIDC device flow) — short-TTL access token held in memory, refresh token in the OS keychain
2. `IdentityAttestationService::attest(AttestInput { token | principal })`:
   - `extract_claims` decodes `sub`, `iss`, `exp`, `roles` (expired → `Expired` error)
   - best-effort `verify` via the configured `TokenVerifier`
   - unreachable/tampered → claim degrades to `Unverified` (authority cleared) — **no error**
3. Claim attaches to `RunInput.identity` / `ApproveInput.approver_id`
4. `IdentityRef::from_claim` redacts (drops `token_ref`) → envelope `identity` block
5. Enterprise consumes the claim for JWKS verification + authorization (never in OSS)
