//! TomlPreconditionRepository — `.rigorix/preconditions.toml` → config.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#config
//! Implements: Contract Freeze — TomlPreconditionRepository stub
//! Issue: #938 (consequence-gating epic — contract freeze); load/parse
//!   behavior closed in ISSUE-CONSEQUENCE-GATING-2
//!
//! Reads the operator-authored precondition file (`.rigorix/preconditions.toml`,
//! same trust surface as `policy.toml` / `permissions.toml`). Loading semantics
//! are frozen:
//!
//! - **Missing file** → `Ok(None)` — fail-open-absent, status quo, no gating
//! - **Corrupt / over safety caps** → `Err(PreconditionError::ConfigInvalid)`
//!   — fail-closed at dispatch; the matching step is refused
//!
//! The method body is a `todo!()` stub — parsing behavior lands with the
//! PreconditionRepository implementation issue.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::precondition::domain::{PreconditionConfig, PreconditionError};

use super::PreconditionRepository;

/// Filesystem precondition-config repository reading
/// `.rigorix/preconditions.toml`.
#[derive(Debug)]
pub struct TomlPreconditionRepository {
    /// Path to the precondition config file (`.rigorix/preconditions.toml`).
    config_path: PathBuf,
}

impl TomlPreconditionRepository {
    /// Create the repository over a config file path.
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
        }
    }

    /// The config file path this repository reads.
    pub fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }
}

#[async_trait]
impl PreconditionRepository for TomlPreconditionRepository {
    async fn load_config(&self) -> Result<Option<PreconditionConfig>, PreconditionError> {
        todo!(
            "ISSUE-CONSEQUENCE-GATING-2: read + parse {:?}, enforce SafetyCaps, missing ⇒ Ok(None)",
            self.config_path
        )
    }
}
