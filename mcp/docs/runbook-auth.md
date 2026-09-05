# Runbook: auth

## Overview

The auth module provides client-side identity attestation for the Rigorix MCP
Gateway (ADR-008). It establishes **who the human is** via the OIDC device flow
(RFC 8628) against any IdP the dev or org configures, holds the long-lived
refresh token in the OS keychain (`KeychainStore`), and produces short-TTL
identity claims (via `TokenProvider` + engine attestation, ADR-012) that flow
into `rigorix_run` (author), `rigorix_approve_execution` (approver), and the
signed audit envelope.

Auth is a **library module** inside the `rigorix-mcp` crate — it exposes three
MCP tools (`rigorix_auth_login/status/logout`) and an optional SSE auth gate.
It is **not** an access-control gate on the agent's tool calls: OSS attests
(who was presented as acting), Enterprise authorizes (ADR-012).

## Startup Sequence

1. **Configuration required**: IdP settings under `.rigorix/auth.toml`
   `[auth]` or environment (`RIGORIX_IDP_ISSUER`, `RIGORIX_IDP_CLIENT_ID`,
   optional `RIGORIX_IDP_CLIENT_SECRET`)
2. **Dependencies available**: the `rigorix-engine` identity module
   (`IdentityAttestationService`) — compiled in; no service to start
3. **Composition**: `AuthServiceFactoryImpl::create` wires `HttpIdpClient`,
   `KeychainStoreImpl`, `InMemoryTokenProvider`, and the engine attestation
   service into `AuthServiceImpl`
4. **Keychain probe**: `KeychainStoreImpl::keychain()` probes the OS keychain
   at construction — headless environments surface `AuthError::Keychain`
   immediately
5. **Tool registration**: the three `rigorix_auth_*` tools register with the
   MCP ToolRegistry like any other tool (ADR-004)
6. **SSE gate** (optional, non-localhost binds only): `SseAuthGateImpl` in
   `none | api_key | idp` mode enforces `mcp.sse.auth`

### Startup Order

```
Configuration → AuthServiceFactory composition → keychain probe →
tool registration → (optional) SSE auth gate
```

### Failure at startup

- **Keychain unavailable** (headless CI): `KeychainStoreImpl::keychain()`
  fails fast. Use the explicit opt-in `KeychainStoreImpl::plaintext(dir)`
  fallback in CI only (documented degraded mode).

## Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `auth.issuer` / `RIGORIX_IDP_ISSUER` | — | OIDC issuer URL (must be HTTPS) |
| `auth.client_id` / `RIGORIX_IDP_CLIENT_ID` | — | OAuth2 client id at the IdP |
| `auth.client_secret` / `RIGORIX_IDP_CLIENT_SECRET` | none | Optional (confidential clients); stored as `Secret` |
| `auth.access_token_ttl_secs` | 900 | Short-TTL access-token lifetime (5–15 min per ADR-008) |
| `mcp.sse.auth` | `none` | SSE transport policy when binding off-localhost: `none`/`idp`/`api_key` |
| `mcp.sse.bind_address` | `127.0.0.1` | Localhost default requires no auth (ADR-005) |

## Graceful Shutdown

The auth module holds no background tasks of its own (device-flow polling is
driven by tool calls). Shutdown procedure:

1. `rigorix_auth_logout` — revoke the refresh token at the IdP (RFC 7009,
   best-effort), delete the keychain credential, clear the in-memory token
2. Drop the composed `AuthService` (no file handles, no sockets kept open;
   `HttpIdpClient`'s reqwest pool closes with the client)
3. Nothing persists beyond the OS keychain credential — a clean exit leaves
   the refresh token intact for the next session (no re-login needed)

## Health Check

There is no standalone `/health` endpoint in this library module — the
gateway's health surface covers it. Operator equivalents:

- **`rigorix_auth_status`** returns `authenticated | expired | unauthenticated`
  plus a redacted claim summary and source marker (`idp_token`,
  `local_principal`, `unverified`)
- **Keychain reachability**: call `KeychainStoreImpl::keychain()` — success
  means the platform store is available
- **IdP reachability**: a login attempt that returns
  `AuthError::Discovery`/`Transport` indicates the IdP is unreachable

## Common Failure Modes

### IdP unreachable (`AuthError::Discovery` / `Transport`)

- **Symptom**: `rigorix_auth_login` fails; status stays `unauthenticated`;
  `poll` reports a transport fault and keeps the flow pending
- **Impact**: local development is **not** blocked — attestation degrades
  explicitly to `IdentitySource::Unverified` (fail-open, ADR-008); runs and
  approvals proceed with an explicit `identity: unverified` envelope marker
- **Recovery**: restore IdP connectivity; retry login. No data loss.

### User denies / device code expires

- **Symptom**: `rigorix_auth_login`/status polling reports `denied`/`expired`
- **Impact**: none — no credentials are persisted on failure
  (`AuthLoginFailed` event with a typed reason)
- **Recovery**: re-run `rigorix_auth_login` (a fresh device code is issued)

### Keychain unavailable

- **Symptom**: `AuthError::Keychain` on store/read/delete; logout cannot
  clear custody
- **Recovery**: desktop — unlock the OS keychain. CI — configure the explicit
  `KeychainStoreImpl::plaintext` fallback directory (degraded mode, never on
  shared machines)

### Access token expired (silent refresh)

- **Symptom**: `rigorix_auth_status` reports `expired`
- **Recovery**: automatic — the next `attest()`/refresh triggers a silent
  refresh-token exchange from the keychain (AC#4). If the refresh token was
  revoked at the IdP, re-login is required

### SSE gate rejects connections (non-localhost only)

- **Symptom**: transport-level `401` before any tool dispatch
- **IdP mode**: bearer signature invalid or IdP unreachable (fail-closed by
  design — a network-exposed gateway denies when it cannot verify)
- **Recovery**: fix the credential or IdP connectivity; for localhost binds
  the gate is never active (ADR-005 default)

## Observability

- **Structured logging**: all domain events (`AuthLoginStarted/Succeeded/
  Failed`, `AuthStatusChecked`, `AuthLoggedOut`) are recorded to the
  `rigorix::auth` tracing target — event payloads are redacted by
  construction (no raw token material anywhere; SpanPrivacy)
- **Tracing**: the auth flow runs inside the gateway's tokio/tracing context;
  `AuthService` operations are await-points suitable for `#[tracing::instrument]`
  wrapping at the handler boundary
- **Correlation**: each login session carries a `session_id` (UUID v4)
  threaded through events
- **Privacy rule**: `Secret<T>` renders as `***REDACTED***` in Debug/Display/
  Serialize — never log a token, device code, or client secret

## Alerts

| Condition | Severity | Action |
|-----------|----------|--------|
| Repeated `AuthError::Keychain` (custody failing) | High | Check OS keychain availability; a CI fallback may be required |
| SSE gate denying valid traffic (IdP mode) | High | IdP JWKS/connectivity issue — verify before rolling back config |
| `access_denied` floods from one user | Low | Human decision point — no automated action |
| IdP unreachable | Medium | Degraded `unverified` attestation is expected; monitor duration |

## Debugging

### Enable debug logging

```bash
RUST_LOG=rigorix::auth=debug,rigorix_mcp=info cargo run --bin rigorix-mcp
```

### Verify IdP reachability + discovery

```bash
curl -fsS https://<issuer>/.well-known/openid-configuration | jq .issuer
```

### Verify the keychain store

Probe via `KeychainStoreImpl::keychain()`; on failure inspect the
`AuthError::Keychain` message (entry service/account in the error context).

### Manual login round trip (mock IdP)

Use `HttpIdpClient` against a loopback mock IdP — plain HTTP is accepted only
for loopback addresses (test policy), production issuers remain HTTPS-only.

## Recovery Procedures

1. **Re-login** (`rigorix_auth_login`): re-establishes the full identity
   lifecycle (new device code → keychain refresh token → in-memory access
   token)
2. **Logout-then-login**: clears any inconsistent custody state
   (keychain + memory) before re-authenticating
3. **Fallback custody (CI)**: point `KeychainStoreImpl::plaintext` at a
   dedicated, user-restricted directory and confirm the startup warning
   (degraded mode is explicit, never silent)
