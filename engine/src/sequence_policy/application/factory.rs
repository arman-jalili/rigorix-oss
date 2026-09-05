//! SequencePolicyFactory — factory interface for constructing sequence-policy
//! service instances.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#ddd-layers
//! Implements: Contract Freeze — SequencePolicyFactory trait
//! Issue: #838 (sequence-policy epic — contract freeze); implementation with
//!   ISSUE-SEQUENCE-POLICY-2 (SequencePolicyService)
//!
//! Factories encapsulate construction of a `SequencePolicyService` with the
//! rule-config repository it evaluates against. The repository is injected at
//! construction time; the config itself is re-read per run from disk (rules
//! may change between runs — prior decisions stand).
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured `SequencePolicyService`
//! - The repository is always injected — never defaulted inside the service
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::sequence_policy::domain::SequencePolicyError;
use crate::sequence_policy::infrastructure::repository::SequencePolicyRepository;

use super::service::SequencePolicyService;

/// Factory for constructing `SequencePolicyService` instances.
#[async_trait]
pub trait SequencePolicyFactory: Send + Sync {
    /// Create a `SequencePolicyService` bound to the given rule-config
    /// repository.
    ///
    /// # Errors
    /// - `SequencePolicyError::Internal` — construction failed
    async fn create(
        &self,
        repository: Box<dyn SequencePolicyRepository>,
    ) -> Result<Box<dyn SequencePolicyService>, SequencePolicyError>;
}
