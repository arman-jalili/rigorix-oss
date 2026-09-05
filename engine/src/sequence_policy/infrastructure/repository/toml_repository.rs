//! TomlSequencePolicyRepository — `.rigorix/sequence-policy.toml` → config.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#configuration
//! Implements: Contract Freeze — TomlSequencePolicyRepository stub
//! Issue: #838 (sequence-policy epic — contract freeze); load/parse behavior
//!   in the fail-closed / fail-open-absent issues
//!
//! Reads the operator-authored rule file (`.rigorix/sequence-policy.toml`,
//! same trust surface as `policy.toml` / `permissions.toml`). Loading
//! semantics are frozen:
//!
//! - **Missing file** → `Ok(None)` — fail-open-absent, status quo, no gating
//! - **Corrupt / over safety caps** → `Err(SequencePolicyError::InvalidConfig`
//!   / `RuleExceedsCaps`) — fail-closed at plan time; the run is refused and
//!   no steps execute
//! - Regex predicates are compiled once per load (resource concern)
//!
//! The method body is a `todo!()` stub — parsing behavior lands with the
//! fail-closed / fail-open-absent implementation issues.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::sequence_policy::domain::{SequencePolicyConfig, SequencePolicyError};

use super::SequencePolicyRepository;

/// Filesystem rule-config repository reading `.rigorix/sequence-policy.toml`.
#[derive(Debug)]
pub struct TomlSequencePolicyRepository {
    /// Path to the rule config file (`.rigorix/sequence-policy.toml`).
    config_path: PathBuf,
}

impl TomlSequencePolicyRepository {
    /// Create the repository over a config file path.
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
        }
    }
}

#[async_trait]
impl SequencePolicyRepository for TomlSequencePolicyRepository {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        todo!(
            "fail-closed/fail-open-absent issues: read + parse {:?}, enforce SafetyCaps",
            self.config_path
        )
    }
}
