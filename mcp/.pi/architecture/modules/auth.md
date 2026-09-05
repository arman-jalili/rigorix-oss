# Auth Module

## Module Status

**Status:** Implemented (epic: auth — #819)
**Last reviewed:** 2026-09-05
**Blueprint Source:** Requirements v1 — 2026-08-28 (approved, BUILT via #820–#827, MRs #828–#834)

## Description

Client-side identity attestation for the Rigorix MCP Gateway. Establishes **who the human is** via OIDC device flow against an IdP (Keycloak, Entra ID, Okta — any OIDC provider the dev or org supplies credentials for), holds the long-lived credential in the OS keychain, and produces short-TTL identity claims that flow into `rigorix_run` (author), `rigorix_approve_execution` (approver), and the signed audit envelope.

This module is **not** an access-control gate on the agent's tool calls — the MCP client is the agent, the untrusted party; authenticating the untrusted party's tool access is theater (ADR-008). Auth in OSS has two legitimate jobs:

1. **Identity for the evidence chain** (primary): verifiable, attributed human identity on runs and approvals — the credibility layer for approval binding
2. **Transport auth for non-localhost SSE** (secondary): an optional gate for team gateways / CI runners exposing the gateway on a network

**The seam (ADR-012):** OSS attests (records who was presented as acting); Enterprise authorizes (JWKS verification, scope/RBAC). This module is the OSS client half; the engine's `identity` module is the shared contract + attestation core.

## Architecture

| Layer | Responsibility | Path |
|-------|---------------|------|
| **Domain** | IdpConfig, TokenStatus, AuthError, DeviceFlowState | `src/auth/domain/` |
| **Application** | AuthService (login/logout/status/refresh), token custody orchestration | `src/auth/application/` |
| **Infrastructure** | IdpClient (OIDC device flow over reqwest), KeychainStore (keyring), TokenProvider | `src/auth/infrastructure/` |
| **Interfaces** | MCP tool handlers (`rigorix_auth_login`, `rigorix_auth_status`, `rigorix_auth_logout`), SSE auth middleware | `src/auth/interfaces/` |

## Related ADRs

| ADR | Relevance |
|-----|-----------|
| [ADR-008](./decisions/ADR-008-auth-client-flow.md) | **New** — OIDC device flow, keychain custody, short-TTL tokens, SSE transport auth |
| [ADR-005](./decisions/ADR-005-authentication-and-authorization.md) | Amended — defines transport trust (stdio trusted parent, localhost SSE); this module extends the SSE boundary with optional IdP/API-key auth for non-localhost binds |
| [ADR-012](../engine/.pi/architecture/decisions/ADR-012-identity-attestation.md) | Defines the OSS-attests / Enterprise-authorizes seam and the `IdentityClaim` contract |
| [ADR-004](./decisions/ADR-004-mcp-protocol-design.md) | Defines MCP tool registration — auth tools register like all others |

## Components

### AuthService (Application)

**Purpose:** Orchestrates the identity lifecycle: login (device flow), refresh, status, logout

**Implementation File:** `src/auth/application/service.rs`

**Key behaviors:**
- `login()`: initiate OIDC device authorization grant — returns `verification_uri` + `user_code` to display; polls the token endpoint; on success stores the refresh token in the OS keychain and returns a short-TTL access token to the in-memory `TokenProvider`
- `refresh()`: exchange refresh token for a new access token (silent, background)
- `status()`: report `TokenStatus::Authenticated | Expired | Unauthenticated` + claim summary
- `logout()`: revoke/clear tokens from keychain and memory
- `attest()`: delegate to engine `IdentityAttestationService` — token → `IdentityClaim`

### IdpClient (Infrastructure)

**Purpose:** OIDC device flow client for any IdP the dev/org configures

**Implementation File:** `src/auth/infrastructure/idp_client.rs`

**Key behaviors:**
- OIDC discovery (`.well-known/openid-configuration`) — issuer, device-authorization endpoint, token endpoint, JWKS
- Device authorization request + token polling (per RFC 8628)
- Configurable via `.rigorix/auth.toml` or env (`RIGORIX_IDP_ISSUER`, `RIGORIX_IDP_CLIENT_ID`, optional `RIGORIX_IDP_CLIENT_SECRET` for confidential clients)
- HTTPS enforced; TLS verification mandatory (matches enterprise proxy policy)

### KeychainStore (Infrastructure)

**Purpose:** Long-lived credential custody — the refresh token lives HERE, never in config files the agent can read

**Implementation File:** `src/auth/infrastructure/keychain_store.rs`

**Key behaviors:**
- OS keychain via `keyring` crate (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- Refresh token is the crown jewel: never written to `.rigorix/` or any readable file
- Fallback: explicit opt-in plaintext file with warning (for CI environments without a keychain) — documented degraded mode

### TokenProvider (Infrastructure)

**Purpose:** Short-TTL access token custody in-memory — what the agent-visible surface can present

**Implementation File:** `src/auth/infrastructure/token_provider.rs`

**Key behaviors:**
- Holds the current access token (default TTL 5–15 min)
- Auto-refresh via the refresh token (keychain)
- Never persists access tokens to disk

### AuthHandler / SSE Auth (Interfaces)

**Purpose:** MCP tool handlers + optional SSE transport gate

**Implementation File:** `src/auth/interfaces/mcp/mod.rs`, `src/auth/interfaces/sse_auth.rs`

**Key behaviors:**
- `rigorix_auth_login` / `rigorix_auth_status` / `rigorix_auth_logout` tool handlers
- SSE auth middleware: when `mcp.sse.bind_address` is non-localhost AND `mcp.sse.auth = "idp" | "api_key"` is set, the SSE transport requires a valid bearer token before routing any tool call
- When auth is not configured, SSE binds localhost-only (existing ADR-005 default) — no behavior change

## Domain Events

| Event | Description | Payload | Consumers |
|-------|-------------|---------|-----------|
| **AuthLoginStarted** | Device flow initiated | `{ session_id, verification_uri, user_code }` | Logger, TUI |
| **AuthLoginSucceeded** | Token exchange completed | `{ session_id, subject, issuer, token_ttl_secs }` | Logger, Metrics |
| **AuthLoginFailed** | Device flow failed (denied/expired/error) | `{ session_id, error_type, reason }` | Logger, Alerts |
| **AuthStatusChecked** | Identity status queried | `{ session_id, status (authenticated/expired/unauthenticated), claim_summary }` | Logger |
| **AuthLoggedOut** | Tokens cleared | `{ session_id, revoked }` | Logger |

## API Endpoints (MCP Tool Schema)

| Method (tool name) | Handler | Input | Output | Auth |
|--------------------|---------|-------|--------|------|
| `rigorix_auth_login` | handle_auth_login | `{}` (or `{ client_id, issuer }` overrides) | `{ status, verification_uri, user_code, expires_in }` | None (bootstraps identity) |
| `rigorix_auth_status` | handle_auth_status | `{}` | `{ status, claim_summary: { subject, issuer, authority, expires_at }, source }` | None (read-only, redacted) |
| `rigorix_auth_logout` | handle_auth_logout | `{}` | `{ status: "logged_out" }` | None (self-service) |

All three tools return **redacted** output — never the raw token.

## Acceptance Criteria

| # | Criterion | Verify In |
|---|-----------|-----------|
| 1 | Device flow: verification_uri + user_code returned; polling succeeds on authorization | unit test |
| 2 | Login denied/expired → `AuthLoginFailed` with typed reason | unit test |
| 3 | Status: authenticated/expired/unauthenticated transitions correct | unit test |
| 4 | Expired access token silently refreshed from keychain refresh token | unit test |
| 5 | Logout clears keychain + memory; subsequent tools run with `identity: absent` | integration test |
| 6 | SSE non-localhost + auth=idp → unauthenticated bearer rejected before tool dispatch | integration test |
| 7 | SSE localhost default → no auth required (regression) | integration test |
| 8 | IdP unreachable → status unauthenticated; run proceeds with explicit `identity: unverified` marker | integration test |
| 9 | Envelope identity block is redacted (no raw token anywhere) | integration test |
| 10 | `rigorix_approve_execution` records approver_id from the claim + token_claims_ref | integration test |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| **Identity Claim** | Attributed, time-bound statement of who was presented as acting (see engine identity module) |
| **Attestation** | Recording the presented identity as evidence — OSS's job |
| **Authorization** | Evaluating whether an identity may act — Enterprise's job (never OSS) |
| **Device Flow** | OIDC authorization grant (RFC 8628) for terminal/CLI login |
| **Refresh Token** | Long-lived credential held in the OS keychain, human-owned |
| **Access Token** | Short-TTL token held in-memory, what the agent-visible surface can present |
| **Attributed Claim** | A recorded fact, not a credential-backed proof — the honest boundary |

## Dependencies

### Depends On
- **Engine — Identity module**: `IdentityClaim`, `IdentityAttestationService` (attestation core)
- **MCP Server**: tool registration, SSE transport lifecycle
- **Configuration**: IdP settings, SSE auth policy

### Used By
- **Execution Tools**: `rigorix_run` (author claim), `rigorix_approve_execution` (approver_id + token_claims_ref)
- **Audit Tools**: identity present in envelopes read back by `rigorix_read_audit`
- **Enterprise Proxy**: identity claims forwarded for Enterprise-side authorization
- **TUI/CLI**: login prompts, status display

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Refresh token theft | OS keychain only; never in readable config/files; agent cannot read it | security-validator |
| Agent holding long-lived authority | Short-TTL access tokens (5–15 min) in-memory only; refresh stays with the human | security-validator |
| Identity spoofing | Claims are attributed, documented as such; envelope marks `identity: unverified` when degraded | security-validator |
| Non-localhost SSE exposure | Optional IdP/API-key gate when binding off-localhost; localhost default unchanged | security-validator |
| Token leakage in logs | Redacted outputs; SpanPrivacy; never log raw tokens | security-validator |
| OSS authorization drift | No scope evaluation in OSS — attestation only; Enterprise authorizes | architecture-validator |
| IdP outage blocks development | Fail-open with explicit `Unverified` marker; never fail-closed for local dev | operations-validator |

## Testing Requirements

| Test Type | Coverage Target | Scenarios |
|-----------|----------------|-----------|
| Unit | 90% | Device-flow state machine; token parse/refresh logic; keychain abstraction (mock) |
| Integration | 80% | Mock IdP: login → status → run with identity → envelope identity block; logout revokes |
| E2E | Manual | Real Keycloak realm (enterprise repo `keycloak/realm-config.json`) |

**Key Test Scenarios:**
1. Device flow: verification_uri + user_code returned; polling succeeds on authorization
2. Login denied/expired → `AuthLoginFailed` with typed reason
3. Status: authenticated/expired/unauthenticated transitions correct
4. Refresh: expired access token silently refreshed from keychain refresh token
5. Logout clears keychain + memory; subsequent tools run with `identity: absent`
6. SSE non-localhost + auth=idp → unauthenticated bearer rejected before tool dispatch
7. SSE localhost default → no auth required (regression)
8. IdP unreachable → attestation degrades to `IdentitySource::Unverified`, run proceeds with explicit marker
9. Envelope identity block is redacted (no raw token anywhere)
10. `rigorix_approve_execution` records approver_id from the claim + token_claims_ref

## Configuration

```toml
# .rigorix/auth.toml
[auth]
# OIDC provider — dev or org supplies these
issuer = "https://idp.example.com/realms/rigorix"
client_id = "rigorix-cli"
# Optional for confidential clients
client_secret = "${RIGORIX_IDP_CLIENT_SECRET}"
# Access token TTL in seconds (agent-visible surface)
access_token_ttl_secs = 900

[mcp.sse]
# Existing transport config
bind_address = "127.0.0.1"
# Optional gate when binding off-localhost: "none" | "idp" | "api_key"
auth = "none"
```

## Implementation Status

All components from this blueprint are implemented per the contract freeze
(#820) and the per-component issues. Behavior is validated by unit,
integration, and contract-shape tests plus deterministic proofing
(`check_auth_contracts.sh`, hardening stage 14).

| Component | Implementation | File | Issue |
|-----------|----------------|------|-------|
| AuthService (Application) | `AuthServiceImpl` (login/poll/status/refresh/logout/attest), `AuthServiceFactoryImpl` | `src/auth/application/{service_impl,factory}.rs` | #821 |
| IdpClient (Infrastructure) | `HttpIdpClient` — OIDC discovery + device flow (RFC 8628/6749/7009/8414) | `src/auth/infrastructure/idp_client_impl.rs` | #822 |
| KeychainStore (Infrastructure) | `KeychainStoreImpl` — OS keychain (`keyring`) + explicit plaintext fallback | `src/auth/infrastructure/keychain_store_impl.rs` | #823 |
| TokenProvider (Infrastructure) | `InMemoryTokenProvider` — short-TTL in-memory custody | `src/auth/infrastructure/token_provider_impl.rs` | #824 |
| AuthHandler / SSE Auth (Interfaces) | `AuthToolHandlerImpl`, `SseAuthGateImpl` (none/api_key/idp, fail-closed) | `src/auth/interfaces/` | #825 |
| Proofing & CI | `check_auth_contracts.sh`, hardening stage 14 | `.pi/scripts/ci/` | #826 |
| Architecture readiness | `docs/runbook-auth.md`, `docs/dr-plan-auth.md` | `docs/` | #827 |

**Operability notes (post-implementation):**

- Tool output is always redacted (SpanPrivacy); the raw token never crosses
  any serialized surface (`Secret<T>` renders `***REDACTED***`)
- Access tokens are short-TTL and in-memory only; refresh authority stays in
  the OS keychain with the human (ADR-008)
- IdP outage degrades attestation to `IdentitySource::Unverified` (fail-open
  for local dev); the non-localhost SSE gate fails closed (it protects a
  network-exposed gateway, not the agent)
- Tool registration and SSE transport middleware wiring remain transport
  concerns of the gateway (SSE transport is currently removed per GAP-A-10;
  the gate is ready for its return)

## Implementation Sequence

| # | Item | Description | Depends On |
|---|------|-------------|-----------|
| 1 | Domain types | IdpConfig, TokenStatus, AuthError, DeviceFlowState | — |
| 2 | IdpClient | OIDC discovery + device flow (RFC 8628) over reqwest | 1 |
| 3 | KeychainStore | `keyring`-based refresh token custody | — |
| 4 | TokenProvider | In-memory short-TTL access token + auto-refresh | 2, 3 |
| 5 | AuthService | login/logout/status/refresh orchestration | 2, 3, 4 |
| 6 | Attestation bridge | Delegate to engine `IdentityAttestationService` | 5, engine identity |
| 7 | MCP handlers | rigorix_auth_login / status / logout | 5 |
| 8 | SSE auth middleware | Optional gate for non-localhost binds | 7 |
| 9 | Execution/Audit wiring | run author, approve approver_id, envelope identity | 6, approval module |

**Delivery status:** rows 1–8 shipped in the auth epic (#820–#827, MRs
#828–#834). Row 9 is downstream integration work in the execution/audit
modules (the claim/attestation boundary is frozen and ready).

## Integration with Execution Tools

- `rigorix_run` gains an optional `identity` parameter (or derives it from the active session): the `IdentityClaim` becomes the envelope's `identity` block and replaces the self-asserted `author` when present
- `rigorix_approve_execution` gains `approver_id` (derived from the active claim when available) + `authority` + optional `decision_context` — feeding the approval binding module (R3)

---

*Last updated: 2026-09-05*
*Module version: 2.0.0 (Implemented)*

---

**Status:** Implemented (epic: auth — #819)
**Implementation priority:** P1 — delivered via #820–#827 (MRs #828–#834); row 9 (Execution/Audit wiring) is downstream integration work in the execution/audit modules
