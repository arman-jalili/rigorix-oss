//! TomlPreconditionRepository — `.rigorix/preconditions.toml` → config.
//!
//! @canonical .pi/architecture/modules/precondition.md#config
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
//! Behavior lands here (ISSUE-CONSEQUENCE-GATING-2).

use std::path::PathBuf;

use async_trait::async_trait;

use crate::precondition::domain::{PreconditionConfig, PreconditionError};
use crate::precondition::infrastructure::redacted_path;

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

    /// Synchronous load used by composition roots to arm the gate at startup.
    ///
    /// Semantics are identical to [`PreconditionRepository::load_config`]:
    /// missing file → `Ok(None)` (fail-open-absent); corrupt / over-cap →
    /// `Err(PreconditionError::ConfigInvalid)` (fail closed); otherwise the
    /// validated config. The per-dispatch async path still re-reads the file
    /// (ADR-017 forbids caching the authority across the T₀→Tₙ gap).
    pub fn load_config_blocking(&self) -> Result<Option<PreconditionConfig>, PreconditionError> {
        let text = match std::fs::read_to_string(&self.config_path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(
                    path = %self.config_path.display(),
                    "precondition: config file absent — fail-open-absent"
                );
                return Ok(None);
            }
            Err(error) => {
                // Absolute-path-free in the record; full path in the local log.
                tracing::warn!(
                    path = %self.config_path.display(),
                    %error,
                    "precondition: config file could not be read"
                );
                return Err(PreconditionError::ConfigInvalid(format!(
                    "failed to read {}: {error}",
                    redacted_path(&self.config_path)
                )));
            }
        };
        parse_config(&self.config_path, &text).map(Some)
    }
}

/// Parse + validate an operator precondition config (shared by the sync and
/// async load paths so arming and per-dispatch reads cannot diverge).
fn parse_config(
    path: &std::path::Path,
    text: &str,
) -> Result<PreconditionConfig, PreconditionError> {
    // Parse the operator schema: `[[preconditions]]` + optional `[gating]`.
    let config: PreconditionConfig = toml::from_str(text).map_err(|error| {
        // A composition root forwards this Display into the fail-closed gate's
        // `unarmed_detail`, which lands in the signed record for every refused
        // step — so the config path must be redacted here, not at the boundary.
        tracing::warn!(
            path = %path.display(),
            %error,
            "precondition: config file failed to parse"
        );
        PreconditionError::ConfigInvalid(format!("parse error in {}: {error}", redacted_path(path)))
    })?;
    // Enforce the safety caps and structural validity — an over-cap or
    // malformed file refuses a matching step like a corrupt one.
    config.validate_with_default_caps()?;
    Ok(config)
}

#[async_trait]
impl PreconditionRepository for TomlPreconditionRepository {
    async fn load_config(&self) -> Result<Option<PreconditionConfig>, PreconditionError> {
        // A MISSING file is fail-open-absent: `Ok(None)` → no preconditions →
        // no gating (status quo). Any other read failure is fail closed.
        let text = match tokio::fs::read_to_string(&self.config_path).await {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(
                    path = %self.config_path.display(),
                    "precondition: config file absent — fail-open-absent"
                );
                return Ok(None);
            }
            Err(error) => {
                // Absolute-path-free in the record; full path in the local log.
                tracing::warn!(
                    path = %self.config_path.display(),
                    %error,
                    "precondition: config file could not be read"
                );
                return Err(PreconditionError::ConfigInvalid(format!(
                    "failed to read {}: {error}",
                    redacted_path(&self.config_path)
                )));
            }
        };

        // Parse the operator schema: `[[preconditions]]` + optional `[gating]`.
        parse_config(&self.config_path, &text).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write `content` to a unique temp file and return its path.
    fn temp_file(content: &str) -> std::path::PathBuf {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("preconditions.toml");
        std::fs::write(&path, content).expect("write temp config");
        // Leak the dir so the file survives for the async read; removed by the
        // OS temp cleaner.
        std::mem::forget(dir);
        path
    }

    const VALID: &str = r#"
[[preconditions]]
id = "beneficiary-eligible"
match = { tool = "payment_execute" }
require_params = ["/beneficiary", "/amount"]
command = ["/opt/rigorix/checks/beneficiary-eligible"]
timeout_ms = 5000

[gating]
release_dependents_on_failure = false
"#;

    #[tokio::test]
    async fn missing_file_is_none() {
        let repository =
            TomlPreconditionRepository::new("/nonexistent/does-not-exist/preconditions.toml");
        assert!(matches!(repository.load_config().await, Ok(None)));
    }

    #[tokio::test]
    async fn valid_file_loads_all_fields() {
        let repository = TomlPreconditionRepository::new(temp_file(VALID));
        let config = repository
            .load_config()
            .await
            .expect("load")
            .expect("present");
        assert_eq!(config.preconditions.len(), 1);
        assert_eq!(config.preconditions[0].id, "beneficiary-eligible");
        assert!(!config.gating.release_dependents_on_failure);
    }

    #[tokio::test]
    async fn malformed_file_fails_closed() {
        let repository = TomlPreconditionRepository::new(temp_file("this is not = valid toml [["));
        let error = repository.load_config().await.expect_err("fail closed");
        assert!(matches!(error, PreconditionError::ConfigInvalid(_)));
        assert!(!error.is_retriable());
    }

    #[tokio::test]
    async fn over_cap_file_fails_closed() {
        // 101 preconditions exceeds the default max_preconditions_per_file=100.
        let mut content = String::new();
        for i in 0..=100 {
            content.push_str(&format!(
                "[[preconditions]]\nid = \"p{i}\"\nmatch = {{ tool = \"payment_execute\" }}\ncommand = [\"/opt/rigorix/checks/p{i}\"]\n"
            ));
        }
        let repository = TomlPreconditionRepository::new(temp_file(&content));
        let error = repository.load_config().await.expect_err("fail closed");
        assert!(matches!(error, PreconditionError::ConfigInvalid(_)));
    }

    #[tokio::test]
    async fn structural_invalidity_fails_closed() {
        // Empty argv[0] is a structural error, not an implicit no-op.
        let repository = TomlPreconditionRepository::new(temp_file(
            r#"
[[preconditions]]
id = "empty-command"
match = { tool = "payment_execute" }
command = []
"#,
        ));
        let error = repository.load_config().await.expect_err("fail closed");
        assert!(matches!(error, PreconditionError::ConfigInvalid(_)));
    }
}
