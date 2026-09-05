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
        // Read the operator-authored rule file. A MISSING file is the
        // fail-open-absent case: `Ok(None)` → no rules → no gating (status
        // quo). Any other read failure is a load error (fail closed).
        let text = match tokio::fs::read_to_string(&self.config_path).await {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(SequencePolicyError::InvalidConfig(format!(
                    "failed to read {}: {e}",
                    self.config_path.display()
                )));
            }
        };

        // Parse the flat operator schema: `fail_closed` (defaults true) at
        // top level + ordered `[[rules]]` tables.
        let config: SequencePolicyConfig = toml::from_str(&text).map_err(|e| {
            SequencePolicyError::InvalidConfig(format!(
                "parse error in {}: {e}",
                self.config_path.display()
            ))
        })?;

        // Enforce the safety caps (rule count, steps per rule, window, regex
        // predicates) — an over-cap file refuses the plan like a corrupt one.
        config.validate_with_default_caps()?;
        Ok(Some(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_policy::domain::{ParamMatchKind, RuleAction};

    /// Write `content` to a unique temp file and return its path.
    fn temp_file(content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rigorix-sp-toml-{}-{}.toml",
            uuid::Uuid::new_v4(),
            std::thread::current().name().unwrap_or("t")
        ));
        std::fs::write(&path, content).expect("write temp config");
        path
    }

    fn conference_file() -> String {
        r#"fail_closed = true

[[rules]]
id = "registration-remove-then-reassign"
name = "No remove-then-reassign of a full event seat"
description = "conference seat"
steps = [
  { tool = "registration_remove", params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
  { tool = "registration_add",    params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
]
window = 3
action = "promote"
"#
        .to_string()
    }

    #[tokio::test]
    async fn missing_file_is_ok_none() {
        let repo = TomlSequencePolicyRepository::new(
            std::env::temp_dir()
                .join("rigorix-absent-")
                .join("no-such-config.toml"),
        );
        let loaded = repo.load_config().await.expect("missing file is Ok(None)");
        assert!(loaded.is_none(), "absent config → no rules → no gating");
    }

    #[tokio::test]
    async fn valid_operator_file_parses_into_config() {
        let path = temp_file(&conference_file());
        let repo = TomlSequencePolicyRepository::new(&path);
        let loaded = repo.load_config().await.expect("load");
        let config = loaded.expect("Some config");
        assert!(config.fail_closed);
        assert_eq!(config.rules.len(), 1);
        let rule = &config.rules[0];
        assert_eq!(rule.id, "registration-remove-then-reassign");
        assert_eq!(rule.steps.len(), 2);
        assert_eq!(rule.steps[0].params[0].kind, ParamMatchKind::Exact);
        assert_eq!(rule.window, Some(3));
        assert_eq!(rule.action, RuleAction::Promote);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn corrupt_file_is_err_invalid_config() {
        let path = temp_file("fail_closed = true\n[[rules]]\nid = 'unterminated");
        let repo = TomlSequencePolicyRepository::new(&path);
        let err = repo.load_config().await.expect_err("corrupt TOML must Err");
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
        assert!(!err.is_retriable());
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn over_cap_file_is_err_rule_exceeds_caps() {
        // 9 step predicates > max_steps_per_rule default (8).
        let mut toml = String::from(
            "[[rules]]\nid = \"over-cap\"\nname = \"n\"\ndescription = \"d\"\nsteps = [\n",
        );
        for i in 0..9 {
            toml.push_str(&format!("  {{ tool = \"tool_{i}\" }},\n"));
        }
        toml.push_str("]\n");
        let path = temp_file(&toml);
        let repo = TomlSequencePolicyRepository::new(&path);
        let err = repo.load_config().await.expect_err("over-cap must Err");
        assert!(matches!(&err,
            SequencePolicyError::RuleExceedsCaps { rule, .. } if rule == "over-cap"
        ));
        assert!(!err.is_retriable());
        let _ = std::fs::remove_file(&path);
    }
}
