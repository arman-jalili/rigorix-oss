//! Interfaces layer for Effect Commitment.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — `EffectCommitmentLedger` port + HTTP contracts
//! Issue: #1113
//!
//! `interfaces/` is the contract consumed by `execution_engine`. It holds the
//! `EffectCommitmentLedger` port (the model the engine programs against) and the
//! HTTP endpoint/error contracts exposed by the opt-in distributed adapter.

#[cfg(test)]
pub mod contract;
pub mod effect_commitment_ledger;
pub mod http;
pub mod provider;

pub use effect_commitment_ledger::EffectCommitmentLedger;
pub use http::{CommitmentErrorResponse, EFFECT_COMMITMENT_ENDPOINTS, EndpointContract};

pub use provider::{EFFECT_IDEMPOTENCY_ENV, EFFECT_IDEMPOTENCY_HEADER};
