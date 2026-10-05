//! PreconditionFactory — factory interface for constructing precondition
//! service instances.
//!
//! @canonical .pi/architecture/modules/precondition.md#ddd-layers
//! Implements: Contract Freeze — PreconditionFactory trait
//! Issue: #938 (consequence-gating epic — contract freeze); implementation with
//!   ISSUE-CONSEQUENCE-GATING-4
//!
//! Factories encapsulate construction of a `PreconditionService` with the
//! config repository and runner it evaluates against. Both ports are injected
//! at construction time; the config is re-read per dispatch (authority is not
//! cached across T₀→Tₙ).
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured `PreconditionService`
//! - Repository and runner are always injected — never defaulted inside the
//!   service
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::precondition::domain::PreconditionError;
use crate::precondition::infrastructure::PreconditionRunner;
use crate::precondition::infrastructure::repository::PreconditionRepository;

use super::service::PreconditionService;

/// Factory for constructing `PreconditionService` instances.
#[async_trait]
pub trait PreconditionFactory: Send + Sync {
    /// Create a `PreconditionService` bound to the given config repository and
    /// runner.
    ///
    /// # Errors
    /// - `PreconditionError::ConfigInvalid` — the config could not arm at
    ///   construction time (fail closed)
    async fn create(
        &self,
        repository: Box<dyn PreconditionRepository>,
        runner: Box<dyn PreconditionRunner>,
    ) -> Result<Box<dyn PreconditionService>, PreconditionError>;
}
