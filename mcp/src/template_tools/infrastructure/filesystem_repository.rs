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
    GovernanceChange, PlanTemplate, ReplaceReport, StepDefinition, TemplateFilter, TemplateSummary,
};
use rigorix_engine::templates::domain::{
    Template as EngineTemplate, TemplateAction, ValidationRule,
};

/// A legacy document's own timestamps, or the host's clock when it has none (#1563).
///
/// This is the read side of the same rule the create door applies, and it is the direction that can
/// silently lie: a converter that stamps unconditionally reports `created_at = now` on every read of
/// the same file, so a template gains a new creation time each time someone looks at it. A stored
/// document that carries its timestamps keeps them; only a document that has none is stamped, and
/// then once, by the host that read it.
///
/// The engine's `Template` type has no timestamp fields at all, which is why these are read from the
/// document and passed in rather than taken from the parsed type.
fn document_timestamps(
    document: &serde_json::Value,
) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    use chrono::{DateTime, Utc};
    let read = |key: &str| {
        document
            .get(key)
            .and_then(|value| value.as_str())
            .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
            .map(|at| at.with_timezone(&Utc))
    };
    let now = Utc::now();
    (
        read("created_at").unwrap_or(now),
        read("updated_at").unwrap_or(now),
    )
}

/// The one conversion from a template **action** to the `(tool, parameters)` a plan step carries.
///
/// #1651: this was a `match` with a `_` arm producing `("run_command", {"command": ""})`, so every
/// action the adapter had not been taught — `git_stage`, `git_commit`, `file_patch`, `ToolCall`,
/// `GitRead`, `LspQuery` — became `sh -c ""`, which **exits 0**. The step reported success, the run
/// reported `Completed`, and nothing had happened: `git_stage` returned `True` with an unchanged
/// index, `git_commit` returned `True` with no commit, `file_patch` returned `True` with the file
/// untouched. A `requires_approval` step paused, a human signed, and the resumed step committed
/// nothing — a governance record attesting a consequence that did not occur.
///
/// The catch-all existed so the conversion kept compiling while the dialect grew. That is exactly
/// what an exhaustive match is for: it should have failed to compile. There is **no `_` arm** now, so
/// a new action variant is a build error (E0004) until it is given a mapping here.
///
/// The mapping is not invented: the engine's own dispatcher is the authority
/// (`engine/src/planning/application/pipeline_impl.rs:307` maps an action to the `(tool, intent)` pair
/// a node executes), and `engine/src/execution_engine/application/service_impl/tools.rs` is the list
/// of tool names it can dispatch. `declared_scope` is carried for the effect-naming tools because
/// #1175 made the approval declare the files a step touches.
///
/// An action the dispatcher cannot execute is **refused**, not converted: a step that will fail at
/// dispatch is not the same thing as a step whose effect is represented, and reporting the first as
/// success is the defect this function exists to remove.
fn action_to_step(action: &TemplateAction) -> Result<(String, serde_json::Value), TemplateError> {
    use serde_json::json;
    let step = match action {
        TemplateAction::RunCommand { command, .. } => {
            // Only `command` is read by the dispatcher (`command_from_intent`); `cwd`, `env` and
            // `timeout_secs` are carried by the action but not honoured on this path, so a document
            // that sets them would be converted into a plan that silently ignores them (#1658).
            ("run_command".to_string(), json!({ "command": command }))
        }
        TemplateAction::FileRead { path } => ("file_read".to_string(), json!({ "path": path })),
        TemplateAction::FileWrite { path, content } => (
            "file_write".to_string(),
            json!({ "path": path, "content": content, "declared_scope": [path] }),
        ),
        TemplateAction::FileAppend { path, content } => (
            "file_append".to_string(),
            json!({ "path": path, "content": content, "declared_scope": [path] }),
        ),
        TemplateAction::FilePatch {
            path,
            insert,
            search,
            before,
            anchor_type,
            anchor_name,
            container,
            position,
        } => {
            let mut params = serde_json::Map::new();
            params.insert("path".to_string(), json!(path));
            params.insert("insert".to_string(), json!(insert));
            if let Some(search) = search {
                params.insert("search".to_string(), json!(search));
                params.insert("before".to_string(), json!(before));
            }
            if let Some(anchor_type) = anchor_type {
                params.insert("anchor_type".to_string(), json!(anchor_type));
            }
            if let Some(anchor_name) = anchor_name {
                params.insert("anchor_name".to_string(), json!(anchor_name));
            }
            if let Some(container) = container {
                params.insert("container".to_string(), json!(container));
            }
            if let Some(position) = position {
                params.insert("position".to_string(), json!(position));
            }
            params.insert("declared_scope".to_string(), json!([path]));
            ("file_patch".to_string(), serde_json::Value::Object(params))
        }
        // The demo form (#1529): the document already names the tool and its arguments, so the step
        // *is* that tool call. This arm is what keeps `tool = "test_tool"` a real call rather than a
        // translated one.
        TemplateAction::ToolCall { tool, parameters } => (tool.clone(), parameters.clone()),
        TemplateAction::GitRead {
            command,
            path,
            max_results,
        } => {
            let mut params = serde_json::Map::new();
            params.insert("command".to_string(), json!(command));
            if let Some(path) = path {
                params.insert("path".to_string(), json!(path));
            }
            params.insert("max_results".to_string(), json!(max_results));
            ("git_read".to_string(), serde_json::Value::Object(params))
        }
        TemplateAction::GitStage { path } => (
            "git_stage".to_string(),
            json!({ "path": path, "declared_scope": [path] }),
        ),
        TemplateAction::GitCommit {
            message,
            auto_stage,
        } => (
            "git_commit".to_string(),
            json!({ "message": message, "auto_stage": auto_stage }),
        ),
        // No arm of the dispatcher executes an LSP query: `lsp_query` is not in its tool list, so a
        // step converted from this action could only fail at dispatch. Refusing here is the honest
        // conversion — the document is representable as a plan only by dropping the step, which is
        // the no-op this issue removed.
        TemplateAction::LspQuery {
            query_type, file, ..
        } => {
            return Err(TemplateError::UnsupportedAction(format!(
                "an `lsp_query` node (query_type {query_type:?} on {file:?}) has no plan-step \
                 equivalent: the dispatcher executes run_command, file_read, file_write, \
                 file_append, file_patch, git_read, git_stage, git_commit and edit_file, and an \
                 LSP query is not one of them. The template was not converted, and no step of it \
                 was dropped — a plan that reports success for work it cannot do is the defect \
                 this refusal replaces (#1651)"
            )));
        }
    };
    Ok(step)
}

/// The governance-relevant difference between two plan documents (#1655).
///
/// Only the three fields that decide what a run does with a step: the tool it calls, the parameters it
/// passes, and whether a human must approve it. A description edit is not a boundary change and is
/// deliberately not reported — a report that lists everything is a report nobody reads.
fn governance_diff(previous: &PlanTemplate, next: &PlanTemplate) -> Vec<GovernanceChange> {
    let mut changes = Vec::new();
    let previous_by_name: std::collections::HashMap<&str, &StepDefinition> = previous
        .steps()
        .iter()
        .map(|step| (step.name(), step))
        .collect();
    for step in next.steps() {
        let Some(before) = previous_by_name.get(step.name()) else {
            changes.push(GovernanceChange {
                step: step.name().to_string(),
                field: "step".to_string(),
                from: "(absent)".to_string(),
                to: format!("{} step", step.tool()),
            });
            continue;
        };
        if before.tool() != step.tool() {
            changes.push(GovernanceChange {
                step: step.name().to_string(),
                field: "tool".to_string(),
                from: before.tool().to_string(),
                to: step.tool().to_string(),
            });
        }
        if before.parameters() != step.parameters() {
            changes.push(GovernanceChange {
                step: step.name().to_string(),
                field: "parameters".to_string(),
                from: before.parameters().to_string(),
                to: step.parameters().to_string(),
            });
        }
        if before.requires_approval() != step.requires_approval() {
            changes.push(GovernanceChange {
                step: step.name().to_string(),
                field: "requires_approval".to_string(),
                from: before.requires_approval().to_string(),
                to: step.requires_approval().to_string(),
            });
        }
    }
    // A step that disappeared took its gate with it.
    for step in previous.steps() {
        if !next.steps().iter().any(|s| s.name() == step.name()) {
            changes.push(GovernanceChange {
                step: step.name().to_string(),
                field: "step".to_string(),
                from: format!("{} step", step.tool()),
                to: "(removed)".to_string(),
            });
        }
    }
    changes
}

/// Convert an engine Template ([[nodes]] format) to an MCP PlanTemplate ([[steps]] format).
///
/// `created_at` / `updated_at` are the document's own values where it has them —
/// [`document_timestamps`] decides — never a fresh clock per read.
fn engine_template_to_plan_template(
    tmpl: &EngineTemplate,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> Result<PlanTemplate, TemplateError> {
    let steps: Vec<StepDefinition> = tmpl
        .nodes
        .iter()
        .map(|node| -> Result<StepDefinition, TemplateError> {
            let (tool, params) = action_to_step(&node.action)?;
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
            // L1 identity gate (F-20260907-05): propagate the node's
            // require_identity flag so rigorix_run refuses the step for
            // unauthenticated callers (conference-demo seat mutations).
            sd = sd.with_require_identity(node.require_identity);
            Ok(sd)
        })
        .collect::<Result<Vec<_>, TemplateError>>()?;

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
        created_at,
        updated_at,
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

    /// Keep the current definition under `.versions/<name>/`, and return the path it was kept at
    /// (#1655).
    ///
    /// The copy is content-addressed by a digest of the bytes plus a timestamp, so two overwrites in
    /// the same second cannot collide, and the same content kept twice is two files — a history, not a
    /// set. It is a **copy**, not a rename: the live file must stay where readers look.
    async fn keep_previous(&self, name: &str) -> Result<String, TemplateError> {
        let live = self.template_path(name);
        let bytes = tokio::fs::read(&live).await.map_err(|e| {
            TemplateError::RepositoryError(format!(
                "Failed to read '{}' before overwriting it: {}",
                live.display(),
                e
            ))
        })?;
        let digest = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()[..8]
                .to_string()
        };
        let epoch = chrono::Utc::now().timestamp();
        let directory = self.base_path.join(".versions").join(name);
        tokio::fs::create_dir_all(&directory).await.map_err(|e| {
            TemplateError::RepositoryError(format!(
                "Failed to create '{}': {}",
                directory.display(),
                e
            ))
        })?;
        let kept = directory.join(format!("{epoch}-{digest}.toml"));
        tokio::fs::write(&kept, &bytes).await.map_err(|e| {
            TemplateError::RepositoryError(format!("Failed to keep '{}': {}", kept.display(), e))
        })?;
        Ok(kept.display().to_string())
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

        // #1658 item 2: the template is operator configuration under `.rigorix/`, and it was written
        // 0644 — readable by every account on the host. The temp file carries the mode into the
        // destination, since the rename is what makes it the destination.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))
                .await
                .map_err(|e| {
                    TemplateError::RepositoryError(format!(
                        "Failed to set owner-only mode on '{}': {}",
                        tmp_path.display(),
                        e
                    ))
                })?;
        }

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

        // Try PlanTemplate ([[steps]] format) first, then fall back to the engine's document
        // parser (#1529): the version rule, the two dialects and the refusals are decided there,
        // and a document the engine refuses is refused here for the same reason and in the same
        // words, rather than by a second deserializer that could disagree with it.
        match PlanTemplate::from_json(json_value.clone()) {
            Ok(pt) => return Ok(pt),
            Err(_) => {
                let document = rigorix_engine::templates::application::template_document::parse_document(
                    &content,
                    Some(name),
                )
                .map_err(|e| {
                    TemplateError::DeserializationFailed(format!(
                        "Template '{}' is neither a plan template nor an engine template document: {e}",
                        name
                    ))
                })?;
                // The document's own timestamps survive the fallback: only a document without them
                // is stamped, and that stamp is the host's clock at the moment of reading.
                let (created_at, updated_at) = document_timestamps(&json_value);
                return engine_template_to_plan_template(
                    &document.template,
                    created_at,
                    updated_at,
                );
            }
        }
    }

    /// Write, keeping the previous definition and reporting what governance-relevant fields moved
    /// (#1655).
    ///
    /// The audit's case: an operator template with `requires_approval = true` paused a run; the agent
    /// then called `rigorix_create_template` with `overwrite: true` and a plan whose step was
    /// `file_write` with `requires_approval: false`; the same run completed with no human in the loop,
    /// and nothing in the record said a gate had been removed.
    ///
    /// Two properties are added, and neither refuses a legitimate edit:
    ///
    /// - the previous bytes are kept under `.versions/<name>/<epoch>-<digest8>.toml`, so "the previous
    ///   definition is recoverable" is a fact about the store and not a promise about the operator's
    ///   backups;
    /// - the per-step diff of `tool`, `parameters` and `requires_approval` is returned, so an
    ///   overwrite that drops an approval requirement cannot be silent.
    async fn create_reporting(
        &self,
        template: PlanTemplate,
        overwrite: bool,
    ) -> Result<ReplaceReport, TemplateError> {
        let name = template.name().to_string();
        // Read the previous definition **before** the write, from the same door a reader uses, so the
        // diff describes the document rather than the file's bytes in a private shape.
        let previous = match self.get(&name).await {
            Ok(previous) => Some(previous),
            Err(TemplateError::NotFound(_)) => None,
            Err(error) => return Err(error),
        };

        let mut report = ReplaceReport::default();
        if let Some(previous) = previous {
            if !overwrite {
                return Err(TemplateError::AlreadyExists(format!(
                    "Template '{name}' already exists. Use overwrite: true to replace."
                )));
            }
            report.replaced_existing = true;
            report.changes = governance_diff(&previous, &template);
            // ADR-026: a change that reduces what is gated is refused, before a version copy is
            // written and before the template file is touched. A refused write that left a version
            // behind would be a record of a change that did not happen.
            //
            // The rule is here, in the store, and not in the tool's handler: the store is the one door
            // every caller goes through, and a rule in the adapter is a rule the next caller does not
            // have.
            let downgrades = report.downgrades();
            if !downgrades.is_empty() {
                return Err(TemplateError::GovernanceDowngrade(format!(
                    "Refusing to overwrite template '{name}': {}. The template was NOT written and \
                     nothing was changed. A template is the gate a run is judged by, so an agent may \
                     not lower it — ask the operator, who can edit .rigorix/templates/{name}.toml on \
                     disk. Adding a step, adding an approval or editing parameters is applied and \
                     reported (ADR-026).",
                    downgrades.join("; ")
                )));
            }
            report.previous_definition = Some(self.keep_previous(&name).await?);
        }

        self.create(template, overwrite).await?;
        Ok(report)
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

    /// A legacy engine document keeps its own timestamps — and does so on every read (#1563).
    ///
    /// The read side is the direction that can lies silently: a converter that stamps unconditionally
    /// reports `created_at = now` for the same file, so the template appears to have been created
    /// again each time someone looked at it. Two reads two seconds apart make a fresh clock
    /// impossible to mistake for the recorded one, and assert the recorded instant rather than merely
    /// asserting the two reads agree with each other.
    #[tokio::test]
    async fn a_legacy_document_keeps_its_own_timestamps_across_reads() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo = FilesystemTemplateRepository::new(dir.path());

        // A legacy engine document ([[nodes]]), carrying the timestamps a host wrote when it first
        // stored it. The engine's `Template` type has no such fields, so they reach the converter
        // only through the document.
        let legacy = r#"schema_version = 1
id = "legacy-plan"
name = "legacy-plan"
description = "Stored in 2000, read ever since"
version = "1.0.0"
created_at = "2000-01-01T00:00:00Z"
updated_at = "2000-01-02T00:00:00Z"

[[nodes]]
id = "inspect"
name = "inspect"
tool = "file_read"

[nodes.parameters]
path = "/tmp/thing"
"#;
        tokio::fs::write(dir.path().join("legacy-plan.toml"), legacy)
            .await
            .expect("write the fixture");

        let first = repo
            .get("legacy-plan")
            .await
            .expect("the legacy document reads");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let second = repo
            .get("legacy-plan")
            .await
            .expect("the legacy document reads again");

        let recorded = chrono::DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
            .expect("the fixture's instant parses")
            .with_timezone(&chrono::Utc);
        let recorded_updated = chrono::DateTime::parse_from_rfc3339("2000-01-02T00:00:00Z")
            .expect("the fixture's instant parses")
            .with_timezone(&chrono::Utc);

        assert_eq!(
            first.created_at(),
            &recorded,
            "the first read must report the document's own creation time, not the clock"
        );
        assert_eq!(
            second.created_at(),
            &recorded,
            "a second read two seconds later must report the recorded instant, not a fresh one"
        );
        assert_eq!(first.updated_at(), second.updated_at());
        assert_eq!(
            first.updated_at(),
            &recorded_updated,
            "the document's updated_at survives the fallback too"
        );
        assert_eq!(second.created_at(), first.created_at());
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
            schema_version: 1,
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
                    require_identity: false,
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
                    require_identity: false,
                    intent: None,
                },
            ],
            tags: vec![],
            category: None,
            author: None,
        };

        let at = chrono::Utc::now();
        let plan = engine_template_to_plan_template(&engine_tmpl, at, at)
            .expect("valid engine template converts");
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

// ---------------------------------------------------------------------------
// #1651 — the read direction is not lossy, and an unrepresentable document is refused
// ---------------------------------------------------------------------------

#[cfg(test)]
mod round_trip_tests {
    use super::*;
    use crate::template_tools::domain::entity::TemplateRepository;

    fn plan() -> PlanTemplate {
        let steps = vec![
            StepDefinition::new(
                "stage".to_string(),
                "git_stage".to_string(),
                serde_json::json!({"path": "a.txt"}),
                false,
                "step one".to_string(),
                None,
            ),
            StepDefinition::new(
                "commit".to_string(),
                "git_commit".to_string(),
                serde_json::json!({"message": "m"}),
                true,
                "step two".to_string(),
                None,
            ),
        ];
        PlanTemplate::new(
            "round-trip".to_string(),
            "a plan".to_string(),
            "1.0.0".to_string(),
            vec!["tag".to_string()],
            steps,
            None,
            HashMap::new(),
            chrono::Utc::now(),
            chrono::Utc::now(),
        )
        .expect("a plan with steps")
    }

    /// What `create` writes, `get` returns — compared as parsed values, because the claim is about
    /// the document and not about the order TOML happened to serialize.
    #[tokio::test]
    async fn create_then_get_returns_the_same_document() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = FilesystemTemplateRepository::new(dir.path());
        let written = plan();
        repo.create(written.clone(), false).await.expect("create");

        let read = repo.get("round-trip").await.expect("get");
        let written_json = serde_json::to_value(&written).expect("serialize");
        let read_json = serde_json::to_value(&read).expect("serialize");
        assert_eq!(
            written_json, read_json,
            "the template changed by being stored and read back"
        );
    }

    /// A legacy `[[nodes]]` document whose action has no plan-step equivalent is refused with the
    /// typed error, not silently converted into a plan that cannot do the work (#1651).
    #[tokio::test]
    async fn a_legacy_document_with_an_unconvertible_action_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = FilesystemTemplateRepository::new(dir.path());
        let legacy = r#"schema_version = 1
id = "lsp-plan"
name = "lsp-plan"
description = "a legacy document"
version = "1.0.0"

[[nodes]]
id = "n1"
name = "query"
[nodes.action]
type = "lsp_query"
query_type = "goto-definition"
file = "src/lib.rs"
line = 1
column = 2
"#;
        std::fs::create_dir_all(dir.path()).expect("dir");
        std::fs::write(dir.path().join("lsp-plan.toml"), legacy).expect("write");

        let error = repo
            .get("lsp-plan")
            .await
            .expect_err("an LSP query has no step equivalent");
        match error {
            TemplateError::UnsupportedAction(message) => {
                assert!(message.contains("lsp_query"), "{message}");
            }
            other => panic!("expected UnsupportedAction, got {other:?}"),
        }
    }

    /// Every action the dispatcher *can* run survives the legacy → plan conversion with its tool and
    /// its fields intact — the defect was that six of them became `run_command ""`.
    #[tokio::test]
    async fn a_legacy_document_of_git_and_patch_actions_converts_to_those_tools() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = FilesystemTemplateRepository::new(dir.path());
        let legacy = r#"schema_version = 1
id = "git-plan"
name = "git-plan"
description = "a legacy document"
version = "1.0.0"

[[nodes]]
id = "n1"
name = "stage"
[nodes.action]
type = "git_stage"
path = "a.txt"

[[nodes]]
id = "n2"
name = "commit"
[nodes.action]
type = "git_commit"
message = "the message"
"#;
        std::fs::create_dir_all(dir.path()).expect("dir");
        std::fs::write(dir.path().join("git-plan.toml"), legacy).expect("write");

        let plan = repo.get("git-plan").await.expect("converts");
        let tools: Vec<String> = plan
            .steps()
            .iter()
            .map(|step| step.tool().to_string())
            .collect();
        assert_eq!(
            tools,
            vec!["git_stage".to_string(), "git_commit".to_string()],
            "each node must keep its own tool"
        );
        assert_eq!(
            plan.steps()[0]
                .parameters()
                .get("path")
                .and_then(|p| p.as_str()),
            Some("a.txt")
        );
        assert_eq!(
            plan.steps()[1]
                .parameters()
                .get("message")
                .and_then(|m| m.as_str()),
            Some("the message")
        );
    }
}

// ---------------------------------------------------------------------------
// #1651 — the conversion is total, and the step it produces is the one the dispatcher reads
// ---------------------------------------------------------------------------

/// One instance of **every** `TemplateAction` variant.
///
/// A literal list, not a loop over the source: the test that uses it asserts this list's length
/// equals the enum's variant count, so a variant added to the enum without an instance here fails a
/// test instead of being quietly uncovered — "the actions we thought of" is how the original defect
/// read.
#[cfg(test)]
fn every_action() -> Vec<TemplateAction> {
    use std::collections::HashMap;
    vec![
        TemplateAction::RunCommand {
            command: "echo hi".to_string(),
            cwd: None,
            timeout_secs: 60,
            env: HashMap::new(),
        },
        TemplateAction::FileRead {
            path: "read.txt".to_string(),
        },
        TemplateAction::FileWrite {
            path: "write.txt".to_string(),
            content: "W".to_string(),
        },
        TemplateAction::FileAppend {
            path: "append.txt".to_string(),
            content: "A".to_string(),
        },
        TemplateAction::FilePatch {
            path: "patched.txt".to_string(),
            insert: "inserted".to_string(),
            search: Some("search".to_string()),
            before: false,
            anchor_type: None,
            anchor_name: None,
            container: None,
            position: None,
        },
        TemplateAction::ToolCall {
            tool: "test_tool".to_string(),
            parameters: serde_json::json!({"k": "v"}),
        },
        TemplateAction::LspQuery {
            query_type: "goto-definition".to_string(),
            file: "src/lib.rs".to_string(),
            line: 1,
            column: 2,
        },
        TemplateAction::GitRead {
            command: "status".to_string(),
            path: None,
            max_results: 50,
        },
        TemplateAction::GitStage {
            path: "staged.txt".to_string(),
        },
        TemplateAction::GitCommit {
            message: "a commit".to_string(),
            auto_stage: false,
        },
    ]
}

#[cfg(test)]
mod conversion_tests {
    use super::*;

    /// The engine's own dispatcher is the authority for "which tools exist" and "which field the
    /// step reads". Both are read from the engine's source rather than restated here, because a copy
    /// of the list is the drift this conversion is supposed to remove.
    const DISPATCH_SOURCE: &str =
        include_str!("../../../../engine/src/execution_engine/application/service_impl/tools.rs");
    const TEMPLATE_SOURCE: &str =
        include_str!("../../../../engine/src/templates/domain/template.rs");

    /// The tool names the dispatcher matches, read from its `match tool_name` block.
    fn dispatchable_tools() -> Vec<String> {
        let block = DISPATCH_SOURCE
            .split("let result = match tool_name {")
            .nth(1)
            .expect("the dispatcher's match block")
            .split("_ => {")
            .next()
            .expect("the arm list");
        let mut names: Vec<String> = Vec::new();
        for line in block.lines() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix('"')
                && let Some(name) = rest.split('"').next()
                && (trimmed.contains("=>"))
            {
                names.push(name.to_string());
            }
        }
        names
    }

    /// The enum's own variant count, so this suite cannot cover "the actions we thought of".
    fn action_variant_count() -> usize {
        let body = TEMPLATE_SOURCE
            .split("pub enum TemplateAction {")
            .nth(1)
            .expect("the action enum")
            .split("\n}")
            .next()
            .expect("the enum body");
        body.lines()
            .filter(|line| {
                let t = line.trim_end();
                t.starts_with("    ")
                    && !t.trim_start().starts_with("//")
                    && !t.trim_start().starts_with('#')
                    && t.ends_with('{')
                    && t.trim_start()
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_uppercase())
            })
            .count()
    }

    #[test]
    fn the_suite_covers_every_variant_of_the_action_enum() {
        let counted = action_variant_count();
        assert_eq!(
            every_action().len(),
            counted,
            "the conversion suite must carry one instance per TemplateAction variant; the enum has \
             {counted} and `every_action()` has {}",
            every_action().len()
        );
        assert!(counted >= 10, "the enum parse found {counted} variants");
    }

    #[test]
    fn every_convertible_action_names_a_tool_the_dispatcher_can_execute() {
        let dispatchable = dispatchable_tools();
        assert!(
            dispatchable.contains(&"file_patch".to_string()),
            "the dispatcher parse found {dispatchable:?}"
        );
        for action in every_action() {
            match action_to_step(&action) {
                Ok((tool, _)) => {
                    if matches!(action, TemplateAction::ToolCall { .. }) {
                        // A demo-form node names its own tool (#1529); the registry, not this list,
                        // decides whether it exists.
                        continue;
                    }
                    assert!(
                        dispatchable.contains(&tool),
                        "{action:?} converted to tool {tool:?}, which no dispatcher arm executes                          — the conversion would produce a step that fails at dispatch: {dispatchable:?}"
                    );
                }
                Err(error) => {
                    // The only allowed refusal is an action the dispatcher genuinely cannot run.
                    assert!(
                        matches!(action, TemplateAction::LspQuery { .. }),
                        "{action:?} was refused but is dispatchable: {error}"
                    );
                }
            }
        }
    }

    /// The step must carry what the **engine's** extractor reads, asserted with that extractor —
    /// not with a second reading of the same field names here.
    #[test]
    fn the_step_carries_the_field_the_engine_reads_for_each_action() {
        use rigorix_engine::tools::application::shell_policy::field_from_intent;

        let cases: Vec<(TemplateAction, &str, &str)> = vec![
            (
                TemplateAction::RunCommand {
                    command: "echo hi".to_string(),
                    cwd: None,
                    timeout_secs: 60,
                    env: std::collections::HashMap::new(),
                },
                "command",
                "echo hi",
            ),
            (
                TemplateAction::FileRead {
                    path: "read.txt".to_string(),
                },
                "path",
                "read.txt",
            ),
            (
                TemplateAction::FileWrite {
                    path: "write.txt".to_string(),
                    content: "W".to_string(),
                },
                "path",
                "write.txt",
            ),
            (
                TemplateAction::FileAppend {
                    path: "append.txt".to_string(),
                    content: "A".to_string(),
                },
                "path",
                "append.txt",
            ),
            (
                TemplateAction::FilePatch {
                    path: "patched.txt".to_string(),
                    insert: "i".to_string(),
                    search: None,
                    before: false,
                    anchor_type: None,
                    anchor_name: None,
                    container: None,
                    position: None,
                },
                "path",
                "patched.txt",
            ),
            (
                TemplateAction::GitRead {
                    command: "log --oneline".to_string(),
                    path: None,
                    max_results: 50,
                },
                "command",
                "log --oneline",
            ),
            (
                TemplateAction::GitStage {
                    path: "staged.txt".to_string(),
                },
                "path",
                "staged.txt",
            ),
            (
                TemplateAction::GitCommit {
                    message: "a commit".to_string(),
                    auto_stage: false,
                },
                "message",
                "a commit",
            ),
        ];

        for (action, field, expected) in cases {
            let (_, params) = action_to_step(&action).expect("convertible");
            let intent = params.to_string();
            assert_eq!(
                field_from_intent(&intent, field),
                expected,
                "{action:?} produced {intent} — the engine reads {field:?} from this intent"
            );
        }
    }

    /// The refusal is typed and explains itself, and it is not a plan with a step withdrawn from it.
    #[test]
    fn an_action_the_dispatcher_cannot_run_is_refused_by_name() {
        let error = action_to_step(&TemplateAction::LspQuery {
            query_type: "find-references".to_string(),
            file: "src/lib.rs".to_string(),
            line: 3,
            column: 4,
        })
        .expect_err("no dispatch arm executes an LSP query");
        let message = error.to_string();
        assert!(message.contains("lsp_query"), "{message}");
        assert!(message.contains("find-references"), "{message}");
        assert!(
            message.contains("not converted"),
            "the refusal must say the document was not converted: {message}"
        );
    }

    /// The defect itself, as a regression test: no action may become the empty shell command.
    #[test]
    fn no_action_converts_to_an_empty_run_command() {
        for action in every_action() {
            if let Ok((tool, params)) = action_to_step(&action) {
                let empty_shell = tool == "run_command"
                    && params.get("command").and_then(|c| c.as_str()) == Some("");
                assert!(
                    !empty_shell,
                    "{action:?} converted to `run_command \"\"`, which exits 0 and reports success \
                     for work that did not happen (#1651)"
                );
            }
        }
    }
}
