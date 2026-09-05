//! Auth — Client-side identity attestation for the Rigorix MCP Gateway.
//!
//! @canonical .pi/architecture/modules/auth.md
//! Implements: Contract Freeze — auth module root
//! ADR-008: auth client flow (OIDC device flow, keychain custody, SSE transport auth)
//! ADR-012: identity attestation seam — OSS attests, Enterprise authorizes
//!
//! Establishes **who the human is** via OIDC device flow (RFC 8628) against an
//! IdP (Keycloak, Entra ID, Okta — any OIDC provider the dev or org supplies
//! credentials for), holds the long-lived credential in the OS keychain, and
//! produces short-TTL identity claims that flow into `rigorix_run` (author),
//! `rigorix_approve_execution` (approver), and the signed audit envelope.
//!
//! This module is **not** an access-control gate on the agent's tool calls
//! (ADR-008): it records attributed identity as evidence, and optionally gates
//! a network-exposed SSE transport (non-localhost binds only, ADR-005).
//!
//! # Module Structure
//!
//! This module follows Clean Architecture with bounded context (DDD):
//!
//! - `auth/domain/` — IdpConfig, TokenStatus, DeviceFlowState, ClaimSummary,
//!   domain events, error types (pure data, serde-stable)
//! - `auth/application/` — AuthService trait (login/logout/status/refresh/
//!   attest), input/output DTOs, factory interfaces
//! - `auth/infrastructure/` — IdpClient, KeychainStore, TokenProvider
//!   interface traits (OIDC transport / credential custody / in-memory token)
//! - `auth/interfaces/` — MCP tool handler contracts
//!   (`rigorix_auth_login/status/logout`) + SSE auth gate contract
//!
//! # Architecture References
//!
//! | Component | Path (per architecture) | Canonical Section |
//! |-----------|------------------------|-------------------|
//! | AuthService (Application) | `src/auth/application/service.rs` | `.pi/architecture/modules/auth.md#authservice-application` |
//! | IdpClient (Infrastructure) | `src/auth/infrastructure/idp_client.rs` | `.pi/architecture/modules/auth.md#idpclient-infrastructure` |
//! | KeychainStore (Infrastructure) | `src/auth/infrastructure/keychain_store.rs` | `.pi/architecture/modules/auth.md#keychainstore-infrastructure` |
//! | TokenProvider (Infrastructure) | `src/auth/infrastructure/token_provider.rs` | `.pi/architecture/modules/auth.md#tokenprovider-infrastructure` |
//! | AuthHandler / SSE Auth (Interfaces) | `src/auth/interfaces/mcp/mod.rs`, `src/auth/interfaces/sse_auth.rs` | `.pi/architecture/modules/auth.md#authhandler--sse-auth-interfaces` |
//!
//! # Dependencies
//!
//! - **Depends on:** Engine — Identity module (`IdentityClaim`,
//!   `IdentityAttestationService` — attestation core, ADR-012); Configuration
//!   (`.rigorix/auth.toml`, env); MCP Server (tool registration)
//! - **Used by:** Execution Tools (`rigorix_run` author claim,
//!   `rigorix_approve_execution` approver_id), Audit Tools (envelope identity),
//!   Enterprise Proxy (identity forwarding), TUI/CLI (login prompts, status)
//!
//! # Contract (Frozen)
//!
//! - All public interfaces are frozen — no additions without ADR approval
//! - Domain types are pure data with serde Serialize/Deserialize
//! - Service traits are async (async-trait) and return domain error types
//! - Refresh tokens are never in the agent-visible surface — KeychainStore only
//! - Access tokens are short-TTL and in-memory only — TokenProvider only
//! - All tool outputs are redacted — no raw token in any serialized surface
//! - OSS attests, never authorizes (no scope/RBAC evaluation)
//! - Method bodies are `todo!()` stubs — behavior lands in implementation issues

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
