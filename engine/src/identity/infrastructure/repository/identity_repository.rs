//! IdentityRepository — durable identity records attached to execution state.
//!
//! @canonical .pi/architecture/modules/identity.md#identityrepository
//! Implements: Contract Freeze — IdentityRepository trait
//! Issue: #700 (identity epic — contract freeze)
//!
//! Persists and loads `IdentityClaim`s alongside `ExecutionState` (via state
//! persistence) for continuity and audit. Raw tokens are stored **by
//! reference** (`token_ref`), never embedded in serialized records.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return the domain error type `IdentityError`
//! - Lookups are keyed by `execution_id` (`Uuid`, matching state persistence)
//! - `save`/`load` never see the raw token — only the claim's `token_ref`
//! - No framework-specific annotations on trait definitions
//!
//! # TODO
//! Concrete persistence (filesystem / state-persistence-backed) lands in
//! ISSUE-IDENTITY-5 (IdentityRepository).

use async_trait::async_trait;
use uuid::Uuid;

use crate::identity::domain::{IdentityClaim, IdentityError};

/// Repository for persisting and retrieving identity claims.
///
/// Implementations store claims alongside execution state — the default is the
/// local filesystem via state persistence (atomic write-rename), with
/// in-memory implementations for tests.
#[async_trait]
pub trait IdentityRepository: Send + Sync {
    /// Persist an identity claim for an execution.
    ///
    /// # Contract
    /// - Stores the claim by `token_ref`, never the raw token
    /// - Idempotent for the same `(execution_id, claim)` pair
    async fn save(&self, execution_id: Uuid, claim: &IdentityClaim) -> Result<(), IdentityError>;

    /// Load the identity claim attached to an execution.
    ///
    /// Returns `Ok(None)` when no identity was recorded for the execution
    /// (identity is optional — `RunInput.author` stays `None`).
    async fn load(&self, execution_id: Uuid) -> Result<Option<IdentityClaim>, IdentityError>;

    /// Delete the identity record for an execution.
    ///
    /// Idempotent — returns `Ok(())` even if no record exists.
    async fn delete(&self, execution_id: Uuid) -> Result<(), IdentityError>;
}
