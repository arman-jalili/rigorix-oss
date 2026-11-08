//! File-system based implementation of PolicyRepository.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: PolicyRepository — file-based TOML loader
//! Issue: #475
//!
//! Loads policy rules from `.rigorix/policy.toml` or a custom path.
//! Supports runtime save via the repository interface.

use async_trait::async_trait;
use std::path::PathBuf;

use crate::policy_engine::domain::{PolicyConfig, PolicyEngineError, PolicyRule};
use crate::policy_engine::infrastructure::repository::PolicyRepository;

/// File-system implementation of PolicyRepository.
///
/// Reads policy rules from a TOML file. The default path is
/// `.rigorix/policy.toml` relative to the workspace root.
pub struct DefaultPolicyRepository {
    config_path: PathBuf,
}

impl DefaultPolicyRepository {
    /// Create a new repository pointing to the given path.
    pub fn new(path: PathBuf) -> Self {
        Self { config_path: path }
    }

    /// Create a new repository at the default `.rigorix/policy.toml` path.
    pub fn default(workspace_root: PathBuf) -> Self {
        Self {
            config_path: workspace_root.join(".rigorix").join("policy.toml"),
        }
    }
}

#[async_trait]
impl PolicyRepository for DefaultPolicyRepository {
    async fn load_config(&self) -> Result<PolicyConfig, PolicyEngineError> {
        let content = tokio::fs::read_to_string(&self.config_path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    // File not found is not an error — return empty config
                    return PolicyEngineError::InvalidConfiguration {
                        detail: format!(
                            "Policy file not found at '{}', using defaults",
                            self.config_path.display()
                        ),
                    };
                }
                PolicyEngineError::RepositoryError {
                    detail: format!(
                        "Failed to read policy file '{}': {}",
                        self.config_path.display(),
                        e
                    ),
                }
            })?;

        toml::from_str(&content).map_err(|e| PolicyEngineError::DeserializationError {
            detail: format!(
                "Failed to parse policy file '{}': {}",
                self.config_path.display(),
                e
            ),
        })
    }

    async fn save_config(&self, config: &PolicyConfig) -> Result<(), PolicyEngineError> {
        let content =
            toml::to_string(config).map_err(|e| PolicyEngineError::DeserializationError {
                detail: format!("Failed to serialize policy config: {}", e),
            })?;

        // #1490: atomic, through the pinned parent handle. The old shape created
        // the directory, then wrote the path — a reader could observe a partially
        // written policy file, which is the file that decides what is allowed.
        write_atomically(&self.config_path, &content).await?;
        Ok(())
    }

    async fn load_rule(&self, name: &str) -> Result<Option<PolicyRule>, PolicyEngineError> {
        let config = self.load_config().await?;
        Ok(config
            .rules
            .into_iter()
            .find(|def| def.name == name)
            .map(|def| PolicyRule {
                name: def.name,
                condition: def.condition,
                action: def.action,
                priority: def.priority,
            }))
    }

    async fn save_rule(&self, rule: &PolicyRule) -> Result<(), PolicyEngineError> {
        let mut config = self.load_config().await.unwrap_or_default();
        let existing = config.rules.iter_mut().find(|r| r.name == rule.name);
        if let Some(existing_rule) = existing {
            existing_rule.condition = rule.condition.clone();
            existing_rule.action = rule.action.clone();
            existing_rule.priority = rule.priority;
        } else {
            config
                .rules
                .push(crate::policy_engine::domain::RuleDefinition {
                    name: rule.name.clone(),
                    condition: rule.condition.clone(),
                    action: rule.action.clone(),
                    priority: rule.priority,
                });
        }
        self.save_config(&config).await
    }

    async fn delete_rule(&self, name: &str) -> Result<(), PolicyEngineError> {
        let mut config = self.load_config().await.unwrap_or_default();
        config.rules.retain(|r| r.name != name);
        self.save_config(&config).await
    }

    async fn list_rules(&self) -> Result<Vec<String>, PolicyEngineError> {
        let config = self.load_config().await.unwrap_or_default();
        Ok(config.rules.into_iter().map(|r| r.name).collect())
    }
}

/// Write `content` to `path` atomically, creating parents (#1490).
async fn write_atomically(path: &std::path::Path, content: &str) -> Result<(), PolicyEngineError> {
    let display = path.display().to_string();
    let path = path.to_path_buf();
    let content = content.to_string();
    tokio::task::spawn_blocking(move || {
        let target = crate::tools::infrastructure::fs_at::resolve_absolute_for_write(&path)?;
        crate::tools::infrastructure::fs_at::write_atomic(&target, content.as_bytes())
    })
    .await
    .map_err(|e| PolicyEngineError::RepositoryError {
        detail: format!("policy write task failed: {e}"),
    })?
    .map_err(|e| PolicyEngineError::RepositoryError {
        detail: format!("Failed to write policy file '{}': {e}", display),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::domain::{PolicyAction, PolicyCondition, RuleDefinition};
    use tempfile::TempDir;

    fn create_test_repository() -> (TempDir, DefaultPolicyRepository) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("policy.toml");
        let repo = DefaultPolicyRepository::new(path);
        (dir, repo)
    }

    #[tokio::test]
    async fn test_load_config_file_not_found() {
        let dir = TempDir::new().unwrap();
        let repo = DefaultPolicyRepository::new(dir.path().join("nonexistent.toml"));
        let result = repo.load_config().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PolicyEngineError::InvalidConfiguration { .. }
        ));
    }

    #[tokio::test]
    async fn test_save_and_load_config() {
        let (_dir, repo) = create_test_repository();
        let config = PolicyConfig::single(RuleDefinition {
            name: "test-rule".to_string(),
            condition: PolicyCondition::LaneCompleted,
            action: PolicyAction::CloseoutLane,
            priority: 10,
        });

        repo.save_config(&config).await.unwrap();
        let loaded = repo.load_config().await.unwrap();
        assert_eq!(loaded.rules.len(), 1);
        assert_eq!(loaded.rules[0].name, "test-rule");
    }

    #[tokio::test]
    async fn test_save_and_load_rule() {
        let (_dir, repo) = create_test_repository();
        let rule = PolicyRule::new(
            "my-rule".to_string(),
            PolicyCondition::StaleBranch,
            PolicyAction::Block {
                reason: "stale".to_string(),
            },
            10,
        );

        repo.save_rule(&rule).await.unwrap();
        let loaded = repo.load_rule("my-rule").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "my-rule");
    }

    #[tokio::test]
    async fn test_delete_rule() {
        let (_dir, repo) = create_test_repository();
        let rule = PolicyRule::new(
            "delete-me".to_string(),
            PolicyCondition::LaneCompleted,
            PolicyAction::CloseoutLane,
            10,
        );
        repo.save_rule(&rule).await.unwrap();
        repo.delete_rule("delete-me").await.unwrap();
        let loaded = repo.load_rule("delete-me").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_list_rules() {
        let (_dir, repo) = create_test_repository();
        // No rules initially
        let names = repo.list_rules().await.unwrap();
        assert!(names.is_empty());
    }
}
