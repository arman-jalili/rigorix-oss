# ADR-008: Auth Client Flow — OIDC Device Flow, Keychain Custody, SSE Transport Auth

**Status:** Proposed
**Date:** 2026-08-28

**Tech Stack:** Rust (mcp crate)

## Context

The Rigorix MCP Gateway has no identity layer: `rigorix_run` records a self-asserted author, `rigorix_approve_execution` records no identity at all, and SSE bound to a non-localhost address is unauthenticated. The engine's identity module (ADR-012) defines the shared `IdentityClaim` contract and attestation core, but the **client flow** — how the human logs in, where the credential lives, and how the transport is secured — is an MCP-crate concern.

Constraints that shape this decision:
- The MCP protocol has no standard auth mechanism; adding one breaks compatibility with AI clients (ADR-005)
- The MCP client (the agent) is the untrusted party — auth is **not** a gate on agent tool calls
- The gateway is a local, offline-first tool — login must work in a terminal without a redirect server
- The long-lived credential must never be readable by the agent (which can read files in the repo)

## Decision

### 1. OIDC device flow (RFC 8628) for login

`rigorix auth login` initiates the device authorization grant against any OIDC provider the dev/org configures (`RIGORIX_IDP_ISSUER`, `RIGORIX_IDP_CLIENT_ID`, optional `RIGORIX_IDP_CLIENT_SECRET`). The user is shown a `verification_uri` + `user_code` (the GitHub CLI / Azure CLI pattern — no redirect server needed). Polling completes the exchange.

This works with the enterprise's existing Keycloak realm (`rigorix-enterprise/keycloak/realm-config.json`) and any commercial IdP (Entra ID, Okta, Auth0).

### 2. Keychain custody for the refresh token

The refresh token (the crown jewel) is stored in the OS keychain via `keyring` — never in `.rigorix/` or any readable file. Access tokens are short-TTL (5–15 min) and held in-memory only. The agent-visible surface can only ever present a short-TTL access token; refresh authority stays with the human.

Explicit opt-in plaintext fallback exists for CI environments without a keychain, documented as a weaker mode with a warning.

### 3. Attestation bridge to the engine

The MCP `auth` module delegates token → `IdentityClaim` conversion to the engine's `IdentityAttestationService` (ADR-012). OSS does no authorization judgment; it records the claim and forwards it to runs, approvals, and the envelope.

### 4. SSE transport auth for non-localhost binds

Default remains localhost-only (ADR-005). When `mcp.sse.bind_address` is non-localhost AND `mcp.sse.auth` is set (`"idp" | "api_key"`), the SSE transport requires a valid bearer token before routing any tool call. This is the **one** legitimate access-control gate in OSS — it protects a network-exposed gateway, not the agent.

### 5. Offline degradation is explicit

IdP unreachable → `rigorix_auth_status` reports `unauthenticated`/`unverified`; runs/approvals continue with `identity: unverified` in the envelope. Never fail-closed for local dev.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **OIDC device flow (chosen)** | Works in a terminal; no redirect server; any OIDC IdP; keychain fits naturally | User must complete a browser step | **Chosen** |
| Password/API-key login | Simpler | No IdP integration; weaker custody; no refresh model | Rejected |
| OSS mints local JWTs post-login | No per-use IdP dependency | Local signing key = theft target (ADR-012 option b) | Rejected |
| MCP-level auth extension | Uniform | Breaks MCP client compatibility; theater against the agent (ADR-005) | Rejected |
| mTLS for SSE | Strongest | Certificate management burden; overkill for localhost default | Rejected (already in ADR-005) |

## Consequences

### Positive
- Identity flows into run/approve/envelope — the credibility layer for approval binding (ADR-011 R3)
- Agent can never read the refresh token; blast radius of a stolen access token is minutes
- SSE exposed on a network can be gated — closes the ADR-005 admitted gap ("any local process can use all tools")
- Works with the enterprise Keycloak realm today

### Negative
- Keychain dependency; CI needs the opt-in plaintext fallback (documented weaker mode)
- Login requires a browser step (device flow) — unavoidable for terminal identity
- No IdP configured → feature is dormant; no behavior change to existing tools (additive)

### Neutral
- Tool list grows by three (`rigorix_auth_login/status/logout`) — usage guide must be updated

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/auth.md` (new — this contract)
- `.pi/architecture/modules/mcp-server.md` (tool registration, SSE lifecycle)
- `.pi/architecture/modules/execution-tools.md` (run author, approve approver_id)
- `.pi/architecture/modules/usage-guide.md` (auth tools documented)
- `.pi/architecture/modules/enterprise-proxy.md` (claims forwarding)
- `.pi/architecture/decisions/ADR-005-authentication-and-authorization.md` (amended — SSE non-localhost auth)
- `engine/.pi/architecture/modules/identity.md` (shared contract — engine)

**Files to Update:**
- `mcp/src/auth/` (new module — device flow, keychain, token provider, MCP handlers, SSE auth)
- `mcp/src/mcp_server/` (registration)
- `mcp/src/execution_tools/` (identity params)
- `engine/src/identity/` (attestation core — separate epic item)

**Canonical References:**
Implementation files should reference: `.pi/architecture/modules/auth.md`

## Validation

**Validators Required:**
- architecture-validator: OSS-attests/Enterprise-authorizes boundary; no authorization judgment in MCP
- security-validator: keychain custody; short-TTL tokens; redacted outputs; SSE gate correctness; no token leakage in logs
- operations-validator: offline behavior; runbook/DR updates; CI plaintext fallback documented
- tests: 10 acceptance criteria in `auth.md`

## References

- Related ADRs: ADR-012 (identity attestation — the seam), ADR-011 (approval binding — consumer), ADR-005 (transport auth — amended)
- External: RFC 8628 (OAuth 2.0 Device Authorization Grant); keyring crate; enterprise Keycloak realm

---

*Decision date: 2026-08-28*
*Decision makers: Rigorix maintainers (with industry review)*
