//! Repository interfaces for the Consequence Gating bounded context.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#ddd-layers
//! Implements: Contract Freeze — PreconditionRepository trait
//! Issue: #938 (consequence-gating epic — contract freeze)
//!
//! # Contract (Frozen)
//! - `load_config` reads the operator precondition config for a dispatch
//!   (`Ok(None)` means **no config file** — fail-open-absent, no gating;
//!   `Err` means corrupt / over-cap — **fail closed**)
//! - The config is re-read per dispatch — never cached across T₀→Tₙ (ADR-017
//!   "no caching of the answer across the gap")
//! - Persistence/location is hidden behind this interface — no caller touches
//!   the TOML format or trust surface directly
//! - The config lives in `.rigorix/**`, which agents are denied write access to
//!   by default (ADR-013 R5)

pub mod toml_repository;

use async_trait::async_trait;

use crate::precondition::domain::{PreconditionConfig, PreconditionError};

pub use toml_repository::TomlPreconditionRepository;

/// Repository for the operator-authored precondition config.
#[async_trait]
pub trait PreconditionRepository: Send + Sync {
    /// Load the precondition config for a dispatch.
    ///
    /// Returns:
    /// - `Ok(Some(config))` — a valid precondition set
    /// - `Ok(None)` — no config file present → no gating (status quo)
    /// - `Err(...)` — config present but corrupt / over safety caps → refuse
    ///   (fail closed)
    ///
    /// # Errors
    /// - `PreconditionError::ConfigInvalid` — the file is unparseable /
    ///   structurally invalid / over-cap
    async fn load_config(&self) -> Result<Option<PreconditionConfig>, PreconditionError>;
}
