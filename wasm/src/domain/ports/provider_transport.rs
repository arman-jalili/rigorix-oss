//! `ProviderTransport` — the operator check transport, answered from fixtures.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: Contract Freeze — ProviderTransport, ProviderCall, ProviderObservation,
//!   Determination
//! Issue: #1294 (contract freeze)
//!
//! The engine may consult an operator check. In the browser there is no network, so
//! the answer comes from the fixture transcript — and the *shape* of the answer is the
//! engine's, not ours.

use serde::{Deserialize, Serialize};

use crate::domain::value::{Params, PortFailure};

/// One call to an operator check.
///
/// # Contract (Frozen)
/// - Names the provider and method the engine declared; carries parameters the
///   policy already selected
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderCall {
    /// Provider name as declared in the scenario.
    pub provider: String,
    /// Method name as declared in the scenario.
    pub method: String,
    /// Arguments for the call.
    pub params: Params,
}

/// The three-valued answer of a check — the engine's vocabulary.
///
/// # Contract (Frozen)
/// - `Error` refuses exactly like `Failed`; it is kept distinct so the record can say
///   *why* (invariant: an unavailable check is never indistinguishable from a passed
///   one)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Determination {
    /// The check passed.
    Passed,
    /// The check ran and said no.
    Failed,
    /// The check could not run.
    Error,
}

/// What a check answered.
///
/// # Contract (Frozen)
/// - `payload` is the provider's own response, retained for evidence; the crate never
///   interprets it
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderObservation {
    /// The determination.
    pub determination: Determination,
    /// The raw response, if any.
    pub payload: Option<serde_json::Value>,
}

/// Answers a check from the fixture transcript.
///
/// # Contract (Frozen)
/// - No network, no filesystem: a call that the transcript does not answer is
///   `PortFailure { port: ProviderTransport, kind: Missing }`
/// - A transcript answer that does not parse is `Malformed`, not `Failed` — the
///   difference is recorded
pub trait ProviderTransport {
    /// Answer one call.
    fn call(&mut self, call: &ProviderCall) -> Result<ProviderObservation, PortFailure>;
}
