# ADR-012: Identity Attestation — OSS Attests, Enterprise Authorizes

**Status:** Accepted (locally validated)
**Date:** 2026-08-28

**Tech Stack:** Rust

## Context

Rigorix's evidence chain lacks attributable human identity. `RunInput.author` is a self-asserted `Option<String>`; `approve_node` carries no identity. The envelope signs execution events but not *who* decided. Meanwhile:

- The industry is moving toward decision governance where "who authorized, when, under what authority" is first-class evidence (Portotify — capability vs authority; SOC 2 change-management evidence; the OpenAI/HF incident — separation of requester and approver)
- The enterprise already has a Keycloak realm (`rigorix-enterprise/keycloak/realm-config.json`) and deferred JWT/OAuth2 for SSO in its ADR-005
- The OSS MCP ADR-005 already correctly rejects MCP-level auth (transport trust) and OAuth2-for-SSE as over-engineering
- The approval-binding work (ADR-011) needs a credible `approver_id`, not a self-claimed string

**The trap this decision avoids:** authenticating the MCP *client* is theater — the client is the agent, the untrusted party. A token handed to the agent to call `run` proves nothing about the human. The security boundary is the *human-held* artifact: the credential (keychain) and the approval (binding).

## Decision

### 1. OSS attests; Enterprise authorizes

- **OSS (this repo)**: records who was presented as acting — an **attributed, time-bound claim**, best-effort verified when online, explicitly marked when not. OSS makes **no authorization judgment** (no scope/role evaluation).
- **Enterprise (rigorix-enterprise)**: performs JWKS verification and authorization — "is this approver allowed to approve a production-switch?" lives there, on top of the existing API-key/scope RBAC and Keycloak realm.

The two halves meet at the `IdentityClaim` type and the signed envelope: OSS produces evidence; Enterprise consumes and enforces. This mirrors the project's existing enforcement/evidence split.

### 2. Attestation + best-effort verification (Option c)

| Option | Behavior | Decision |
|--------|----------|----------|
| (a) Verify at the edge (JWKS) | OSS fetches IdP JWKS per use | Rejected — breaks offline-first; no IdP in OSS deployments; per-approval network dependency |
| (b) OSS mints local tokens | Post-exchange local signing keys | Rejected — more key management, more theft surface |
| **(c) Attestation + best-effort** | Extract claims; record the raw token **by reference**; verify when online; off-band verification otherwise | **Chosen** |

### 3. Offline policy: fail-open with an explicit marker

No IdP reachable → claims degrade to `IdentitySource::Unverified`; runs/approvals continue; the envelope explicitly marks `identity: unverified` (or `absent`). **Never fail-closed on identity for a local developer tool.** This deliberately differs from Portotify's egress doctrine (block-on-unavailable) — that doctrine governs production egress control; identity attestation for local dev is not egress. The distinction is documented so the two are never conflated.

### 4. Identity is a claim, not a proof

`IdentityClaim` is an attributed, time-bound statement: evidence of who was presented as authorizing, not proof of who the person is. Same discipline as `authority`: captured fact, not judgment. If real authentication is required (e.g., for approving production migrations), that is an **authentication boundary** — a separate concern, out of scope for OSS, enforceable at the Enterprise layer.

### 5. Token custody: human owns the long-lived credential

- Refresh token → OS keychain (human-held, agent-unreadable)
- Access token → in-memory only, short TTL (5–15 min), agent-visible surface presents this
- A stolen agent-visible token has minimal blast radius and no refresh power
- Credential substitution at approval is caught by `token_claims_ref` binding (ADR-011 R3)

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **Attestation + best-effort (chosen)** | Offline-first; honest boundary; no key management in OSS | Identity is a claim, not a proof | **Chosen** |
| Verify-at-edge JWKS | Stronger per-use guarantee | Network dependency per run/approve; breaks offline; no IdP in OSS | Rejected |
| OSS mints local tokens | No IdP dependency post-login | Local signing key = theft target; key lifecycle burden | Rejected |
| MCP-level auth extension (gate the agent) | Simple mental model | The client IS the untrusted party — theater; breaks MCP client compatibility (ADR-005) | Rejected |
| OSS evaluates scope/authorization | Convenient | Re-implements enterprise scope engine in the wrong crate; breaks decision-point/enforcement-point separation (Portotify) | Rejected |

## Consequences

### Positive
- Approver identity becomes verifiable evidence — the credibility layer for approval binding (ADR-011 R3)
- Envelope answers "who, when, under what authority" with an IdP-anchored claim
- Offline-first preserved; no OSS key management; no agent-visible long-lived secrets
- Clean OSS/Enterprise seam — Enterprise consumes claims for authorization (existing Keycloak + scope RBAC)

### Negative
- Identity is attributed, not authenticated — must be documented honestly everywhere
- Degraded mode (`Unverified`) is possible and must be explicit, never silent
- Keychain dependency for real custody; opt-in plaintext fallback for CI is a documented weaker mode
- Envelope gains `identity` block (additive, serde-defaulted)

### Neutral
- `author: Option<String>` is retained for backward compatibility; `identity: Option<IdentityClaim>` supersedes it when present

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/identity.md` (new — this contract, engine)
- `.pi/architecture/modules/auth.md` (new — MCP client flow)
- `.pi/architecture/modules/approval.md` (approver identity consumption)
- `.pi/architecture/modules/orchestrator.md` (RunInput.identity)
- `.pi/architecture/modules/audit.md` (envelope identity block)
- `mcp/.pi/architecture/decisions/ADR-005-authentication-and-authorization.md` (amended — SSE non-localhost auth)
- `mcp/.pi/architecture/modules/enterprise-proxy.md` (claims forwarding)

**Files to Update:**
- `engine/src/identity/` (new module — shared contract + attestation core)
- `mcp/src/auth/` (new module — client flow: device authorization, keychain, MCP tools, SSE auth)
- `engine/src/orchestrator/application/dto/` (RunInput identity)
- `engine/src/audit/domain/envelope.rs` (identity block)
- `mcp/src/execution_tools/` (run author, approve approver_id)

**Canonical References:**
Implementation files should reference: `.pi/architecture/modules/identity.md` (engine), `.pi/architecture/modules/auth.md` (mcp)

## Validation

**Validators Required:**
- architecture-validator: OSS-attests/Enterprise-authorizes boundary; no authorization judgment in OSS
- security-validator: keychain custody; short-TTL access tokens; redacted claims; degradation explicitness
- operations-validator: offline behavior; runbook/DR updates
- tests: 7 acceptance criteria in `identity.md` + 10 in `auth.md`

## References

- Related ADRs: ADR-011 (approval binding — consumes identity), ADR-005 (OSS MCP transport auth — amended), ADR-005 (enterprise — JWT deferred, now the authorization half)
- External: OIDC device flow (RFC 8628); Portotify — "Capability Is Not Authority"; SOC 2 change-management evidence requirements

---

*Decision date: 2026-08-28*
*Decision makers: Rigorix maintainers (with industry review)*


## Validation Evidence (2026-09-06 — local, full-stack)

Status moved from Proposed to **Accepted (locally validated)** after an
end-to-end run on the unreleased local build (not published crates) against
a real infrastructure stack:

- **conference-demo** (github.com/arman-jalili/conference-demo, private):
  docker postgres conference registry + docker Keycloak (RFC 8628 device
  flow), driven through rigorix-mcp over stdio — exit 0, every scene DB-verified.
- Coverage of this ADR in that run: Identity attestation: the organizer's RFC 8628 device-flow login produced an attested claim summary (issuer/subject) bound into the run (scene 4-5); envelope author + identity blocks verified..
- Engine lib 2020/2020; workspace clippy `-D warnings`; mcp conformance 7/7;
  local CI 107/107 (the record for the parent epic).
- Still NOT validated: a public/enterprise deployment (this repo's "ship
  gate") — crates.io publish and a production dashboard+IdP session remain
  future work. Local-only acceptance is the honest ceiling of this flip.

