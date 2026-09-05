# DR Plan: auth

## Overview

The auth module holds exactly **one** piece of long-lived state: the **refresh
token** stored in the OS keychain (`KeychainStore`, ADR-008). Access tokens are
short-TTL and in-memory only (`TokenProvider`) — they self-expire and are
recreated by silent refresh. There is no database, no config file with
secrets, and no replicated service state.

This plan covers loss, corruption, and unavailability of that single
credential, plus the degraded operational modes the module is designed for
(IdP outage, keychain absence in CI).

## Backup Strategy and Schedule

| Asset | Backup mechanism | Schedule | Owner |
|-------|------------------|----------|-------|
| Refresh token (OS keychain) | **No copy is taken by design.** The refresh token is human-owned, per-machine, per-human — it is a bearer credential for the human's IdP session. Backing it up defeats ADR-008 (crown jewel never in readable/portable storage) | n/a | n/a |
| `.rigorix/auth.toml` (IdP **configuration** — public client id/issuer only, no secrets) | Standard VCS | On change | Repo owner |
| CI plaintext fallback dir (explicit opt-in, degraded) | Excluded from backups and agents; treated as disposable | n/a | CI operator |

**RPO for the refresh token: 0 by design** — re-authentication is a
first-class, low-friction operation (`rigorix_auth_login`), so no attempt is
made to reconstruct a lost credential from backup.

## Restore Procedure

### Lost / corrupted refresh token (keychain entry deleted or unreadable)

1. Detect: `rigorix_auth_status` → `unauthenticated`; attest fails with
   `AuthError::NotAuthenticated`; `refresh` fails the same way
2. Restore: run `rigorix_auth_login` — the device flow re-issues a refresh
   token and re-populates the keychain
3. Verify: `rigorix_auth_status` → `authenticated` with a claim summary;
   `rigorix_run`/`rigorix_approve_execution` carry the attested identity

### CI plaintext fallback file deleted

1. Detect: `AuthError::NotAuthenticated` on refresh; `get_refresh_token`
   returns `None`
2. Restore: re-run the login step in the CI job (device flow is interactive —
   for unattended CI prefer a dedicated IdP client/principal or accept
   `identity: unverified` attestation)

### `.rigorix/auth.toml` lost

1. Detect: `AuthError::Configuration` on composition/login
2. Restore: from VCS or re-create from environment variables
   (`RIGORIX_IDP_ISSUER`, `RIGORIX_IDP_CLIENT_ID`)

## Failover Plan

The auth module is **single-instance, single-human**: each gateway process
owns one identity session per machine. There is no multi-node state to fail
over.

| Scenario | Behavior | Plan |
|----------|----------|------|
| Primary machine lost | Refresh token lost with the keychain | Human re-runs `rigorix_auth_login` on the replacement machine (RTO = one device-flow interaction) |
| IdP primary realm down | Discovery/transport errors; attestation degrades to `Unverified` (fail-open for local dev) | Development proceeds with explicit `identity: unverified` markers; runs/approvals remain auditable as degraded (ADR-008). No failover needed — the gateway keeps working |
| IdP realm migrated (new issuer) | Config issuer no longer matches | Update `.rigorix/auth.toml` issuer/client_id; re-login once |

## RTO / RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RPO (refresh token) | 0 — no backup needed | Loss is recoverable by a first-class re-login |
| RPO (access token) | 0 — in-memory, self-expiring | Recreated by silent refresh from the keychain refresh token |
| RTO (keychain credential loss) | < 5 minutes (human device flow) | One `rigorix_auth_login` interaction |
| RTO (IdP outage) | 0 for local development | Fail-open degradation to `IdentitySource::Unverified` (ADR-008); SSE gate (non-localhost) fails closed by design |

## Failure Modes and Recovery Matrix

| Failure | Impact | Recovery | Owner |
|---------|--------|----------|-------|
| Keychain service unavailable | Cannot store/read refresh token | Desktop: unlock keychain. CI: explicit plaintext fallback (degraded) | Operator |
| Refresh token revoked at IdP | Silent refresh fails (`RefreshFailed`/`NotAuthenticated`) | Re-login | Human |
| Device flow denied/expired | Login incomplete; nothing persisted | Re-run login | Human |
| IdP unreachable | Status `unauthenticated`/`unverified`; SSE `idp` gate denies | Restore IdP connectivity | Operator |
| SSE gate misconfig (non-localhost + `idp` without JWKS) | Transport refuses valid traffic (`NotConfigured`) | Fix `jwks_uri` availability / config | Operator |
| Access token expiry | Status `expired` | Automatic silent refresh (AC#4) | System |

## DR Testing

Regular DR verification is intentionally light because the module's state is
a single, recoverable credential:

1. **Quarterly**: delete the keychain refresh token on a dev machine and
   confirm `rigorix_auth_login` restores full identity within minutes
2. **Per CI change**: run the plaintext-fallback keychain integration test
   (headless) and the auth contract proofing stage
   (`bash .pi/scripts/ci/stage_auth_proofing.sh`)
3. **On IdP config change**: run one manual login + status round trip and
   confirm the envelope identity block is redacted (no raw token anywhere)

## Escalation

- Auth module defects (unexpected errors, custody failures) → repository
  issue with the failing `AuthError` variant; severity High for keychain or
  SSE-gate failures
- IdP-side incidents → human/org IdP operations; the module is designed to
  degrade gracefully (attestation) or fail closed (non-localhost SSE gate) —
  never silently
