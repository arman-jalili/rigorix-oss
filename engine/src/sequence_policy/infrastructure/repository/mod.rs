//! Repository interfaces for the Sequence Policy bounded context.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#ddd-layers
//! Implements: Contract Freeze — SequencePolicyRepository trait
//! Issue: #838 (sequence-policy epic — contract freeze)
//!
//! # Contract (Frozen)
//! - `load_config` reads the rule config for a run; `Ok(None)` means **no
//!   config file** (fail-open-absent — status quo, no gating), `Err` means
//!   corrupt/over-cap config (**fail closed** at plan time)
//! - Persistence/location is hidden behind this interface — no caller touches
//!   the file format or trust surface directly
//! - Rule authorship is admin-controlled (R5): the config lives on the same
//!   trust surface as `policy.toml` / `permissions.toml`; executing agents are
//!   denied `.rigorix/**` writes by the default permission config (R5,
//!   permission issue)

pub mod toml_repository;

use async_trait::async_trait;

use crate::sequence_policy::domain::{SequencePolicyConfig, SequencePolicyError};

pub use toml_repository::TomlSequencePolicyRepository;

/// Repository for the sequence-policy rule config.
#[async_trait]
pub trait SequencePolicyRepository: Send + Sync {
    /// Load the rule config for a run.
    ///
    /// Returns:
    /// - `Ok(Some(config))` — a valid rule set
    /// - `Ok(None)` — no config file present → no sequence gating (status quo)
    /// - `Err(...)` — config present but corrupt or over safety caps → the
    ///   plan must be refused (fail closed, when `fail_closed` is enabled)
    ///
    /// # Errors
    /// - `SequencePolicyError::InvalidConfig` — the file is unparseable /
    ///   structurally invalid
    /// - `SequencePolicyError::RuleExceedsCaps` — a rule exceeds the safety
    ///   caps
    /// - `SequencePolicyError::Internal` — IO failure reading the file
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError>;
}
