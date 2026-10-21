//! HTTP endpoint contracts for the contested-store adapter.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#httpeffectledger
//! Implements: ISSUE-CONTRACT-FREEZE — REST endpoint + error response contracts
//! Issue: #1113
//!
//! These constants and records freeze the wire contract between
//! `HttpEffectLedger` and the enterprise commitment service. They are
//! framework-agnostic: the adapter maps them onto `reqwest`; no server framework
//! type leaks here.
//!
//! # Transport discipline (Frozen)
//! - Bounded response body
//! - No redirects (no redirect amplification)
//! - Explicit deadline on every call
//! - Credentials are never logged or emitted in error messages
//! - Every ambiguity maps to `CommitmentError::Unavailable`
//!
//! # Error format (Frozen)
//! ```json
//! { "code": "not_ambiguous", "message": "…", "record_id": "…" }
//! ```

use serde::{Deserialize, Serialize};

use crate::effect_commitment::domain::{CommitmentError, EffectRecordId};

/// Path for recording a write-ahead intent.
pub const BEGIN_PATH: &str = "/v1/effects/intents";

/// Path template for settling a dispatched effect (`{id}` = record UUID).
pub const SETTLE_PATH: &str = "/v1/effects/{id}/settle";

/// Path template for marking an effect ambiguous (`{id}` = record UUID).
pub const AMBIGUOUS_PATH: &str = "/v1/effects/{id}/ambiguous";

/// Path template for resolving an ambiguous effect (`{id}` = record UUID).
pub const RESOLVE_PATH: &str = "/v1/effects/{id}/resolve";

/// Path for querying records (by record id or DAG via query parameters).
pub const QUERY_PATH: &str = "/v1/effects";

/// Path for listing unsettled effects.
pub const AMBIGUOUS_QUERY_PATH: &str = "/v1/effects/ambiguous";

/// A frozen endpoint contract.
///
/// `request_schema` / `response_schema` name the JSON shapes defined in
/// `application::dto`, `domain`, and this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointContract {
    /// HTTP method (`"GET"`, `"POST"`, ...).
    pub method: &'static str,
    /// Path or path template.
    pub path: &'static str,
    /// Name of the request DTO (`"-"` when there is no body).
    pub request_schema: &'static str,
    /// Name of the response DTO / domain type.
    pub response_schema: &'static str,
}

/// The complete set of endpoints the HTTP adapter may call.
pub const EFFECT_COMMITMENT_ENDPOINTS: &[EndpointContract] = &[
    EndpointContract {
        method: "POST",
        path: BEGIN_PATH,
        request_schema: "BeginEffectInput",
        response_schema: "BeginEffectOutput",
    },
    EndpointContract {
        method: "POST",
        path: SETTLE_PATH,
        request_schema: "SettleDefiniteInput",
        response_schema: "SettleDefiniteOutput",
    },
    EndpointContract {
        method: "POST",
        path: AMBIGUOUS_PATH,
        request_schema: "MarkAmbiguousInput",
        response_schema: "MarkAmbiguousOutput",
    },
    EndpointContract {
        method: "POST",
        path: RESOLVE_PATH,
        request_schema: "ResolveEffectInput",
        response_schema: "ResolveEffectOutput",
    },
    EndpointContract {
        method: "GET",
        path: QUERY_PATH,
        request_schema: "-",
        response_schema: "QueryEffectOutput",
    },
    EndpointContract {
        method: "GET",
        path: AMBIGUOUS_QUERY_PATH,
        request_schema: "-",
        response_schema: "QueryEffectOutput",
    },
];

/// The frozen HTTP error body.
///
/// # Fields (Frozen)
/// - `code: String` — stable discriminator from `CommitmentError::code()`
/// - `message: String` — human-readable message, no credentials
/// - `record_id: Option<EffectRecordId>` — the affected record, when known
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitmentErrorResponse {
    /// Stable discriminator (`already_settled`, `not_ambiguous`, …).
    pub code: String,

    /// Human-readable message.
    pub message: String,

    /// The affected record, when known.
    pub record_id: Option<EffectRecordId>,
}

impl CommitmentErrorResponse {
    /// Build an error body from a typed error.
    pub fn from_error(error: &CommitmentError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
            record_id: error.record_id(),
        }
    }
}
