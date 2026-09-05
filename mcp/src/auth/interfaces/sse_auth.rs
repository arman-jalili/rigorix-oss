//! SSE auth gate contract (optional non-localhost transport auth).
//!
//! @canonical .pi/architecture/modules/auth.md#sse-auth
//! Implements: Contract Freeze — SseAuthGate, SseAuthMode, SseAuthDecision
//! ADR-008 §4, ADR-005 (amended): SSE transport auth for non-localhost binds
//!
//! When `mcp.sse.bind_address` is non-localhost AND `mcp.sse.auth` is set
//! (`"idp" | "api_key"`), the SSE transport must require a valid credential
//! before routing any tool call. This is the **one** legitimate access-control
//! gate in OSS — it protects a network-exposed gateway, not the agent.
//!
//! The gate is deliberately framework-agnostic: no axum types appear in this
//! contract. The transport (SSE layer) adapts its middleware to this trait.
//!
//! # Contract (Frozen)
//!
//! - Default remains localhost-only with no auth (ADR-005 regression-safe)
//! - `mode()` reflects the configured policy: `none` | `idp` | `api_key`
//! - `authorize()` is a pure decision — a rejected credential is a `Deny`
//!   decision, never an error; errors mean the gate cannot evaluate
//! - IdP mode validates an RFC 6750 bearer access token via the IdP
//!   (best-effort; IdP unreachable → transport refuses to start)
//! - ApiKey mode validates a configured static key presented as `X-API-Key`

use serde::{Deserialize, Serialize};

use crate::auth::domain::error::SseAuthError;

/// SSE transport auth policy (`mcp.sse.auth`).
///
/// Serialized values match the configuration schema exactly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SseAuthMode {
    /// No transport auth — localhost-only binds (default, ADR-005).
    #[default]
    None,
    /// Validate an OIDC bearer access token against the configured IdP.
    Idp,
    /// Validate a configured static API key (`X-API-Key` header).
    ApiKey,
}

impl SseAuthMode {
    /// True when the gate requires credentials before routing.
    pub fn is_enforced(&self) -> bool {
        !matches!(self, SseAuthMode::None)
    }
}

/// Decision produced by an [`SseAuthGate`] for one request.
///
/// A rejected credential is a decision — not an error — so transport code can
/// respond `401 Unauthorized` uniformly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseAuthDecision {
    /// Credential validated — request may proceed.
    Allow,
    /// Credential missing/invalid — respond 401 with the reason.
    Deny {
        /// Human-readable reason (safe for the client to see).
        reason: String,
    },
}

/// Optional gate for network-exposed SSE transports.
///
/// Implemented over the IdP client (mode `Idp`) or a configured static key
/// (mode `ApiKey`). When `mode()` is `None` the gate is inert — transports
/// must only invoke it when a non-localhost bind is configured.
#[async_trait::async_trait]
pub trait SseAuthGate: Send + Sync {
    /// The configured enforcement mode.
    fn mode(&self) -> SseAuthMode;

    /// Evaluate one inbound SSE connection request.
    ///
    /// - IdP mode: `authorization_header` carries `Bearer <access token>`
    ///   (RFC 6750); `api_key_header` is ignored
    /// - ApiKey mode: `api_key_header` carries the `X-API-Key` value;
    ///   `authorization_header` is ignored
    ///
    /// # Errors
    /// - `SseAuthError::NotConfigured` — mode enabled without required config
    /// - `SseAuthError::IdpUnreachable` — cannot validate in IdP mode
    async fn authorize(
        &self,
        authorization_header: Option<&str>,
        api_key_header: Option<&str>,
    ) -> Result<SseAuthDecision, SseAuthError>;
}
