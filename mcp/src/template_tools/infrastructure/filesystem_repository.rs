//! FilesystemTemplateRepository — Concrete implementation of TemplateRepository.
//!
//! @canonical .pi/architecture/modules/template-tools.md#filesystem-repository
//! Implements: TemplateRepository contract — filesystem-backed atomic TOML storage
//!
//! Stores plan templates as TOML files in a configurable directory.
//! Uses atomic writes (temp file + fsync + rename) for data safety.
//! Template names are filesystem-safe.
//!
//! # Implementation Details
//!
//! - Template files are stored at `{base_path}/{name}.toml`
//! - Writes: write to `.{name}.tmp` → fsync → rename to `{name}.toml`
//! - Concurrent writes protected by `tokio::sync::Mutex`
//! - Template names validated to only contain `[a-zA-Z0-9_-]`
//!
//! # Contract (Frozen)
//!
//! - All public methods match TemplateRepository trait exactly
//! - All errors are wrapped in `TemplateError` variants
//! - Thread-safe (Send + Sync) via Arc<Mutex<>>

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::template_tools::domain::entity::TemplateRepository;
use crate::template_tools::domain::error::TemplateError;
use crate::template_tools::domain::value::{
    PlanTemplate, StepDefinition, TemplateFilter, TemplateSummary,
};
use rigorix_engine::templates::domain::{
    Template as EngineTemplate, TemplateAction, ValidationRule,
};

/// Convert an engine Template ([[nodes]] format) to an MCP PlanTemplate ([[steps]] format).
fn engine_template_to_plan_template(tmpl: &EngineTemplate) -> Result<PlanTemplate, TemplateError> {
    let steps: Vec<StepDefinition> = tmpl
        .nodes
        .iter()
        .map(|node| {
            let (tool, params) = match &node.action {
                TemplateAction::RunCommand { command, .. } => {
                    ("run_command", serde_json::json!({"command": command}))
                }
                TemplateAction::FileRead { path } => {
                    ("file_read", serde_json::json!({"path": path}))
                }
                TemplateAction::FileWrite { path, content } => (
                    "file_write",
                    serde_json::json!({"path": path, "content": content}),
                ),
                TemplateAction::FileAppend { path, content } => (
                    "file_append",
                    serde_json::json!({"path": path, "content": content}),
                ),
                _ => ("run_command", serde_json::json!({"command": ""})),
            };
            let mut sd = StepDefinition::new(
                node.name.clone(),
                tool.to_string(),
                params,
                node.requires_approval,
                format!("Step: {}", node.name),
                None,
            );
            sd.set_evaluate_score(
                node.validate
                    .iter()
                    .any(|v| matches!(v, ValidationRule::ScoredEvaluation)),
            );
            sd
        })
        .collect();

    // Panic is acceptable here: if we parsed a valid engine template, steps are non-empty.
    let now = chrono::Utc::now();
    // GAP-L-08: PlanTemplate::new returns a typed error on empty steps —
    // propagate it instead of panicking on the conversion path.
    PlanTemplate::new(
        tmpl.id.clone(),
        tmpl.description.clone(),
        "1.0.0".to_string(),
        tmpl.tags.clone(),
        steps,
        None,
        HashMap::new(),
        now,
        now,
    )
}

/// Filesystem-backed implementation of TemplateRepository.
///
/// Stores templates as TOML files in a base directory.
/// All writes are atomic: write to temp file → fsync → rename.
///
/// # Invariants
///
/// - Base directory is created on first write if it doesn't exist
/// - Template names are validated to be filesystem-safe
/// - Concurrent writes are serialized via a mutex
#[derive(Clone)]
pub struct FilesystemTemplateRepository {
    /// Base path where template TOML files are stored.
    base_path: PathBuf,

    /// Mutex for serializing concurrent writes.
    write_lock: Arc<Mutex<()>>,
}

impl FilesystemTemplateRepository {
    /// Create a new FilesystemTemplateRepository.
    ///
    /// # Arguments
    /// * `base_path` - Directory where template TOML files are stored.
    ///
    /// The base directory is created lazily on first write.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Get the file path for a template name.
    fn template_path(&self, name: &str) -> PathBuf {
        self.base_path.join(format!("{}.toml", name))
    }

    /// Get the temp file path for atomic writes.
    fn temp_path(&self, name: &str) -> PathBuf {
        self.base_path.join(format!(".{}.tmp", name))
    }

    /// Ensure the base directory exists.
    async fn ensure_dir(&self) -> Result<(), TemplateError> {
        tokio::fs::create_dir_all(&self.base_path)
            .await
            .map_err(|e| {
                TemplateError::RepositoryError(format!(
                    "Failed to create templates directory '{}': {}",
                    self.base_path.display(),
                    e
                ))
            })
    }

    /// Perform an atomic write: temp file → fsync → rename.
    async fn atomic_write(&self, name: &str, content: &str) -> Result<(), TemplateError> {
        let tmp_path = self.temp_path(name);
        let final_path = self.template_path(name);

        // Write to temp file
        tokio::fs::write(&tmp_path, content).await.map_err(|e| {
            TemplateError::RepositoryError(format!(
                "Failed to write temp file '{}': {}",
                tmp_path.display(),
                e
            ))
        })?;

        // fsync the temp file
        let file = tokio::fs::File::open(&tmp_path).await.map_err(|e| {
            TemplateError::RepositoryError(format!(
                "Failed to open temp file '{}' for fsync: {}",
                tmp_path.display(),
                e
            ))
        })?;
        file.sync_all().await.map_err(|e| {
            TemplateError::RepositoryError(format!(
                "Failed to fsync temp file '{}': {}",
                tmp_path.display(),
                e
            ))
        })?;

        // Atomic rename
        tokio::fs::rename(&tmp_path, &final_path)
            .await
            .map_err(|e| {
                TemplateError::RepositoryError(format!(
                    "Failed to rename '{}' to '{}': {}",
                    tmp_path.display(),
                    final_path.display(),
                    e
                ))
            })?;

        Ok(())
    }
}

#[async_trait]
impl TemplateRepository for FilesystemTemplateRepository {
    async fn list(&self, filter: &TemplateFilter) -> Result<Vec<TemplateSummary>, TemplateError> {
        let mut summaries = Vec::new();

        // Read directory entries
        let mut entries = match tokio::fs::read_dir(&self.base_path).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Directory doesn't exist yet — no templates
                return Ok(Vec::new());
            }
            Err(e) => {
                return Err(TemplateError::RepositoryError(format!(
                    "Failed to read templates directory '{}': {}",
                    self.base_path.display(),
                    e
                )));
            }
        };

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            TemplateError::RepositoryError(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();

            // Only process .toml files (not .tmp files)
            if path.extension().map(|ext| ext != "toml").unwrap_or(true) {
                continue;
            }

            // Skip temp files
            let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if file_name.starts_with('.') {
                continue;
            }

            // Read and parse the template
            match self.get(file_name).await {
                Ok(template) => {
                    let summary = TemplateSummary::new(
                        template.name().to_string(),
                        template.description().to_string(),
                        template.version().to_string(),
                        template.tags().to_vec(),
                        template.steps().len(),
                        *template.updated_at(),
                    );

                    // Apply filter
                    if filter_matches(&summary, filter) {
                        summaries.push(summary);
                    }
                }
                Err(e) => {
                    // Skip unparseable files with a warning
                    tracing::warn!(
                        "Skipping unparseable template file '{}': {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            }

            // Apply limit
            if summaries.len() >= filter.limit() {
                break;
            }
        }

        Ok(summaries)
    }

    async fn get(&self, name: &str) -> Result<PlanTemplate, TemplateError> {
        let path = self.template_path(name);

        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TemplateError::NotFound(format!(
                    "Template '{}' not found at '{}'",
                    name,
                    path.display()
                ))
            } else {
                TemplateError::RepositoryError(format!(
                    "Failed to read template '{}': {}",
                    path.display(),
                    e
                ))
            }
        })?;

        // Parse TOML to JSON first, then deserialize through PlanTemplate
        let toml_value: toml::Value = toml::from_str(&content).map_err(|e| {
            TemplateError::DeserializationFailed(format!(
                "Invalid TOML in template '{}': {}",
                name, e
            ))
        })?;

        let json_value = serde_json::to_value(&toml_value).map_err(|e| {
            TemplateError::DeserializationFailed(format!(
                "Failed to convert TOML to JSON for template '{}': {}",
                name, e
            ))
        })?;

        // Try PlanTemplate ([[steps]] format) first, then fall back to EngineTemplate ([[nodes]] format)
        match PlanTemplate::from_json(json_value.clone()) {
            Ok(pt) => return Ok(pt),
            Err(_) => {
                // Not a [[steps]] template — try [[nodes]] format via engine Template
                let engine_tmpl: EngineTemplate = toml::from_str(&content).map_err(|e| {
                    TemplateError::DeserializationFailed(format!(
                        "Template '{}' is neither [[steps]] nor [[nodes]] format: {}",
                        name, e
                    ))
                })?;
                return engine_template_to_plan_template(&engine_tmpl);
            }
        }
    }

    async fn create(&self, template: PlanTemplate, overwrite: bool) -> Result<(), TemplateError> {
        let name = template.name().to_string();

        // Validate name is filesystem-safe
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(TemplateError::InvalidName(format!(
                "Template name '{}' contains invalid characters. Use only alphanumeric, underscore, and hyphen.",
                name
            )));
        }

        // Acquire write lock
        let _lock = self.write_lock.lock().await;

        // Check if exists (unless overwrite)
        if !overwrite {
            let path = self.template_path(&name);
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                return Err(TemplateError::AlreadyExists(format!(
                    "Template '{}' already exists. Use overwrite: true to replace.",
                    name
                )));
            }
        }

        // Ensure directory exists
        self.ensure_dir().await?;

        // Serialize to TOML
        let json_value = serde_json::to_value(&template).map_err(|e| {
            TemplateError::SerializationFailed(format!(
                "Failed to serialize template '{}': {}",
                name, e
            ))
        })?;

        // Convert through intermediate TOML Value for proper formatting
        let toml_value: toml::Value = serde_json::from_value(json_value).map_err(|e| {
            TemplateError::SerializationFailed(format!(
                "Failed to convert template '{}' to TOML: {}",
                name, e
            ))
        })?;

        let toml_string = toml::to_string(&toml_value).map_err(|e| {
            TemplateError::SerializationFailed(format!(
                "Failed to format template '{}' as TOML: {}",
                name, e
            ))
        })?;

        // Atomic write
        self.atomic_write(&name, &toml_string).await
    }

    async fn delete(&self, name: &str) -> Result<(), TemplateError> {
        let path = self.template_path(name);

        let _lock = self.write_lock.lock().await;

        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Err(TemplateError::NotFound(format!(
                "Template '{}' not found",
                name
            )));
        }

        tokio::fs::remove_file(&path).await.map_err(|e| {
            TemplateError::RepositoryError(format!(
                "Failed to delete template '{}': {}",
                path.display(),
                e
            ))
        })?;

        Ok(())
    }

    async fn exists(&self, name: &str) -> Result<bool, TemplateError> {
        let path = self.template_path(name);
        tokio::fs::try_exists(&path).await.map_err(|e| {
            TemplateError::RepositoryError(format!(
                "Failed to check existence of template '{}': {}",
                path.display(),
                e
            ))
        })
    }
}

/// Check if a template summary matches the given filter criteria.
fn filter_matches(summary: &TemplateSummary, filter: &TemplateFilter) -> bool {
    // Tag filter: template must have ALL specified tags
    if let Some(filter_tags) = filter.tags() {
        for tag in filter_tags {
            if !summary.tags().contains(tag) {
                return false;
            }
        }
    }

    // Search filter: matches against name or description (case-insensitive)
    if let Some(search) = filter.search() {
        let search_lower = search.to_lowercase();
        let name_match = summary.name().to_lowercase().contains(&search_lower);
        let desc_match = summary.description().to_lowercase().contains(&search_lower);
        if !name_match && !desc_match {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_tools::domain::value::StepDefinition;
    use rigorix_engine::templates::domain::TemplateNode;

    /// Create a minimal valid PlanTemplate for testing.
    fn test_template(name: &str) -> PlanTemplate {
        let step = StepDefinition::new(
            "step-1".into(),
            "test_tool".into(),
            serde_json::json!({}),
            false,
            "Test step".into(),
            None,
        );
        let now = chrono::Utc::now();
        PlanTemplate::new(
            name.into(),
            "Test description".into(),
            "1.0.0".into(),
            vec![],
            vec![step],
            None,
            std::collections::HashMap::new(),
            now,
            now,
        )
        .expect("Failed to create test template")
    }

    #[tokio::test]
    async fn test_create_and_get_template() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        let template = test_template("hello-world");
        repo.create(template, false)
            .await
            .expect("Create should succeed");

        let retrieved = repo.get("hello-world").await.expect("Get should succeed");
        assert_eq!(retrieved.name(), "hello-world");
        assert_eq!(retrieved.steps().len(), 1);
    }

    #[tokio::test]
    async fn test_get_nonexistent_template() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        let err = repo.get("nonexistent").await.unwrap_err();
        assert!(
            matches!(err, TemplateError::NotFound(_)),
            "Expected NotFound, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_duplicate_without_overwrite() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        let template = test_template("dup");
        repo.create(template.clone(), false)
            .await
            .expect("First create should succeed");

        let err = repo.create(template, false).await.unwrap_err();
        assert!(
            matches!(err, TemplateError::AlreadyExists(_)),
            "Expected AlreadyExists, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_duplicate_with_overwrite() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        let template1 = test_template("overwrite-me");
        repo.create(template1, false)
            .await
            .expect("First create should succeed");

        let template2 = test_template("overwrite-me");
        repo.create(template2, true)
            .await
            .expect("Overwrite should succeed");
    }

    #[tokio::test]
    async fn test_list_empty_directory() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        let filter = TemplateFilter::default();
        let results = repo.list(&filter).await.expect("List should succeed");
        assert!(results.is_empty(), "Expected empty list");
    }

    #[tokio::test]
    async fn test_list_with_templates() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        repo.create(test_template("alpha"), false)
            .await
            .expect("Create should succeed");
        repo.create(test_template("beta"), false)
            .await
            .expect("Create should succeed");

        let filter = TemplateFilter::default();
        let results = repo.list(&filter).await.expect("List should succeed");
        assert_eq!(results.len(), 2, "Expected 2 templates");
    }

    #[tokio::test]
    async fn test_list_with_search_filter() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        repo.create(test_template("rust-project"), false)
            .await
            .expect("Create should succeed");
        repo.create(test_template("python-script"), false)
            .await
            .expect("Create should succeed");

        let filter = TemplateFilter::new(None, Some("rust".into()), None);
        let results = repo.list(&filter).await.expect("List should succeed");
        assert_eq!(results.len(), 1, "Expected 1 template matching 'rust'");
        assert_eq!(results[0].name(), "rust-project");
    }

    #[tokio::test]
    async fn test_list_with_limit() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        repo.create(test_template("a"), false).await.unwrap();
        repo.create(test_template("b"), false).await.unwrap();
        repo.create(test_template("c"), false).await.unwrap();

        let filter = TemplateFilter::new(None, None, Some(2));
        let results = repo.list(&filter).await.expect("List should succeed");
        assert_eq!(results.len(), 2, "Expected 2 templates with limit=2");
    }

    #[tokio::test]
    async fn test_delete_template() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        repo.create(test_template("to-delete"), false)
            .await
            .expect("Create should succeed");

        repo.delete("to-delete")
            .await
            .expect("Delete should succeed");

        let exists = repo
            .exists("to-delete")
            .await
            .expect("Exists check should succeed");
        assert!(!exists, "Template should be deleted");
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        let err = repo.delete("nonexistent").await.unwrap_err();
        assert!(
            matches!(err, TemplateError::NotFound(_)),
            "Expected NotFound, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_exists() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        assert!(!repo.exists("test").await.unwrap());
        repo.create(test_template("test"), false)
            .await
            .expect("Create should succeed");
        assert!(repo.exists("test").await.unwrap());
    }

    #[tokio::test]
    async fn test_atomic_write_preserves_data() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        let template = test_template("atomic-test");
        repo.create(template, false)
            .await
            .expect("Create should succeed");

        // Read raw file to verify it's valid TOML
        let path = dir.path().join("atomic-test.toml");
        let content = tokio::fs::read_to_string(&path)
            .await
            .expect("Should read file");
        assert!(
            content.contains("atomic-test"),
            "File should contain template name"
        );

        // Verify temp files are cleaned up
        let tmp_path = dir.path().join(".atomic-test.tmp");
        assert!(
            !tokio::fs::try_exists(&tmp_path).await.unwrap_or(false),
            "Temp file should be cleaned up"
        );
    }

    #[tokio::test]
    async fn test_invalid_name() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        let template = test_template("../path-traversal");
        let err = repo.create(template, false).await.unwrap_err();
        assert!(
            matches!(err, TemplateError::InvalidName(_)),
            "Expected InvalidName for path traversal, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_template_repository_is_send_sync() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());
        let arc: Arc<dyn TemplateRepository> = Arc::new(repo);
        let _ = arc;
    }

    #[test]
    fn test_nodes_format_propagates_requires_approval() {
        // Fix 2 regression: [[nodes]] templates must preserve the approval
        // flag through the engine-template → plan-template conversion, so a
        // migration runbook can gate its destructive step (migrate → ALTER
        // TABLE) behind human sign-off.
        let engine_tmpl = EngineTemplate {
            id: "db-migration".to_string(),
            name: "db-migration".to_string(),
            description: "Migration runbook".to_string(),
            version: "1.0.0".to_string(),
            parameters: vec![],
            nodes: vec![
                TemplateNode {
                    id: "validate".to_string(),
                    name: "validate".to_string(),
                    depends_on: vec![],
                    action: TemplateAction::RunCommand {
                        command: "true".to_string(),
                        cwd: None,
                        timeout_secs: 30,
                        env: Default::default(),
                    },
                    description: None,
                    retry: Default::default(),
                    validate: vec![],
                    requires_approval: false,
                    intent: None,
                },
                TemplateNode {
                    id: "migrate".to_string(),
                    name: "migrate".to_string(),
                    depends_on: vec!["validate".to_string()],
                    action: TemplateAction::RunCommand {
                        command: "psql -c 'ALTER TABLE ...'".to_string(),
                        cwd: None,
                        timeout_secs: 60,
                        env: Default::default(),
                    },
                    description: None,
                    retry: Default::default(),
                    validate: vec![],
                    requires_approval: true,
                    intent: None,
                },
            ],
            tags: vec![],
            category: None,
            author: None,
        };

        let plan = engine_template_to_plan_template(&engine_tmpl).expect("valid engine template converts");
        let steps = plan.steps();
        assert_eq!(steps.len(), 2);
        assert!(
            !steps[0].requires_approval(),
            "validate step must not require approval"
        );
        assert!(
            steps[1].requires_approval(),
            "migrate step must require approval after conversion"
        );
    }
}
