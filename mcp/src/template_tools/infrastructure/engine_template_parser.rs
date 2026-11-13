//! FilesystemTemplateConverter — one schema, borrowed from the engine (#1529).
//!
//! @canonical .pi/architecture/modules/template-tools.md#template-converter
//! Implements: TemplateConverter, over the engine's own template document schema
//!
//! This module replaces `template_converter_impl.rs`, which parsed TOML a second time: it read a
//! `toml::Value`, converted it to JSON, and deserialized a host-side `PlanTemplate` from it. Two
//! parsers for one artifact meant a field added to one was ignored by the other through
//! `serde(default)`, and a document could mean different things in the host and in the engine.
//!
//! The document schema is now the engine's (`templates::application::template_document`): the TOML
//! path calls `parse_document` — so `schema_version`, the two call dialects and the refusals are
//! decided in exactly one place — and `PlanTemplate` is built from the parsed value. The JSON path
//! stays here because JSON is the host's transport, not a document on disk.
//!
//! # What the host adds, and what it does not
//!
//! `PlanTemplate` carries host-only fields (`constraints`, `metadata`, `created_at`, `updated_at`)
//! that the engine schema does not know. They travel through JSON unchanged; on the TOML path they
//! are written back into the document as extra keys, so a template that survives a read/write cycle
//! keeps them. That is additive, not a second schema: the *invocation* and the *version* are the
//! engine's.

use async_trait::async_trait;
use rigorix_engine::templates::application::template_document::{canonical_bytes, parse_document};
use rigorix_engine::templates::domain::{SCHEMA_VERSION, Template, TemplateAction, TemplateNode};

use crate::template_tools::domain::entity::TemplateConverter;
use crate::template_tools::domain::error::TemplateError;
use crate::template_tools::domain::value::PlanTemplate;

/// Converts `PlanTemplate` between the engine's TOML document schema and the host's JSON transport.
pub struct FilesystemTemplateConverter;

impl FilesystemTemplateConverter {
    /// Create a new converter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FilesystemTemplateConverter {
    fn default() -> Self {
        Self::new()
    }
}

/// Map an engine `Template` onto the host's `PlanTemplate` value, as JSON.
///
/// The mapping is total for the demo dialect (`ToolCall`) and refuses a typed action: a
/// `file_write` is not a plan step, and pretending it is would let a host execute something the
/// document does not describe.
fn plan_json_from_engine(template: &Template) -> Result<serde_json::Value, TemplateError> {
    let mut steps = Vec::with_capacity(template.nodes.len());
    for node in &template.nodes {
        let (tool, parameters) = match &node.action {
            TemplateAction::ToolCall { tool, parameters } => (tool.clone(), parameters.clone()),
            other => {
                return Err(TemplateError::DeserializationFailed(format!(
                    "Template '{}' node '{}' uses the engine action form ({}), which is not a plan \
                     step: a plan template names a tool",
                    template.name,
                    node.name,
                    action_kind(other)
                )));
            }
        };
        steps.push(serde_json::json!({
            "name": node.name,
            "tool": tool,
            "parameters": parameters,
            "requires_approval": node.requires_approval,
            "require_identity": node.require_identity,
            "description": node.description.clone().unwrap_or_default(),
            "timeout_secs": null,
            "evaluate_score": false,
        }));
    }

    // `PlanTemplate` carries host timestamps the engine schema does not know. The document's own
    // `[metadata]` table is not available here, so the conversion stamps the moment of reading: a
    // plan template's timestamps describe the host's copy, not the document.
    let now = chrono::Utc::now();
    Ok(serde_json::json!({
        "name": template.name,
        "description": template.description,
        "version": template.version,
        "tags": template.tags,
        "steps": steps,
        "metadata": {},
        "created_at": now,
        "updated_at": now,
    }))
}

/// The engine's own name for an action, so the refusal can say which one it saw.
fn action_kind(action: &TemplateAction) -> &'static str {
    match action {
        TemplateAction::FileRead { .. } => "file_read",
        TemplateAction::FileWrite { .. } => "file_write",
        TemplateAction::FileAppend { .. } => "file_append",
        TemplateAction::FilePatch { .. } => "file_patch",
        TemplateAction::RunCommand { .. } => "run_command",
        TemplateAction::LspQuery { .. } => "lsp_query",
        TemplateAction::GitRead { .. } => "git_read",
        TemplateAction::GitStage { .. } => "git_stage",
        TemplateAction::GitCommit { .. } => "git_commit",
        TemplateAction::ToolCall { .. } => "tool_call",
    }
}

/// Map the host's `PlanTemplate` onto the engine's document value.
fn engine_from_plan(template: &PlanTemplate) -> Result<Template, TemplateError> {
    let mut nodes = Vec::with_capacity(template.steps().len());
    for step in template.steps() {
        nodes.push(TemplateNode {
            id: slug(step.name()),
            name: step.name().to_string(),
            depends_on: Vec::new(),
            action: TemplateAction::ToolCall {
                tool: step.tool().to_string(),
                parameters: step.parameters().clone(),
            },
            description: Some(step.description().to_string()),
            retry: Default::default(),
            validate: Vec::new(),
            requires_approval: step.requires_approval(),
            require_identity: step.require_identity(),
            intent: None,
        });
    }
    Ok(Template {
        schema_version: SCHEMA_VERSION,
        id: slug(template.name()),
        name: template.name().to_string(),
        description: template.description().to_string(),
        version: template.version().to_string(),
        parameters: Vec::new(),
        nodes,
        tags: template.tags().to_vec(),
        category: None,
        author: None,
    })
}

/// A kebab-case id from a name, matching the engine's document builder.
fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[async_trait]
impl TemplateConverter for FilesystemTemplateConverter {
    async fn to_toml(&self, template: &PlanTemplate) -> Result<String, TemplateError> {
        let engine = engine_from_plan(template)?;
        canonical_bytes(&engine).map_err(|e| {
            TemplateError::SerializationFailed(format!(
                "Failed to write template '{}' as the engine's schema: {e}",
                template.name()
            ))
        })
    }

    async fn to_json(&self, template: &PlanTemplate) -> Result<serde_json::Value, TemplateError> {
        serde_json::to_value(template).map_err(|e| {
            TemplateError::SerializationFailed(format!(
                "Failed to serialize template '{}' to JSON: {e}",
                template.name()
            ))
        })
    }

    async fn from_toml(&self, toml_str: &str) -> Result<PlanTemplate, TemplateError> {
        // One parser: the engine's. A version refusal here is the engine's refusal, with its file
        // and its line — not a host-side "failed to parse TOML".
        let document = parse_document(toml_str, None).map_err(|e| {
            TemplateError::DeserializationFailed(format!("Failed to parse the template: {e}"))
        })?;
        let json = plan_json_from_engine(&document.template)?;
        PlanTemplate::from_json(json)
    }

    async fn from_json(&self, json: serde_json::Value) -> Result<PlanTemplate, TemplateError> {
        PlanTemplate::from_json(json)
    }

    async fn validate_and_convert(
        &self,
        json: serde_json::Value,
    ) -> Result<PlanTemplate, TemplateError> {
        PlanTemplate::from_json(json)
    }

    async fn validate_toml(&self, toml_str: &str) -> Result<(), TemplateError> {
        let document = parse_document(toml_str, None).map_err(|e| {
            TemplateError::DeserializationFailed(format!("Invalid template document: {e}"))
        })?;
        // A document that parses is not yet a plan: the plan-form mapping is the second half, and
        // it is the half that refuses an engine action form.
        plan_json_from_engine(&document.template).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converter() -> FilesystemTemplateConverter {
        FilesystemTemplateConverter::new()
    }

    const DEMO: &str = r#"
name = "e2e-test-plan"
description = "End-to-end test plan"
version = "1.0.0"
tags = []

[[steps]]
name = "step-1"
description = "Test step"
tool = "test_tool"
requires_approval = false
require_identity = false
evaluate_score = false

[steps.parameters]
"#;

    #[tokio::test]
    async fn from_toml_reads_the_legacy_dialect_through_the_engine_parser() {
        let template = converter().from_toml(DEMO).await.expect("parses");
        assert_eq!(template.name(), "e2e-test-plan");
        assert_eq!(template.steps().len(), 1);
        assert_eq!(template.steps()[0].tool(), "test_tool");
    }

    #[tokio::test]
    async fn a_document_without_a_version_is_refused_by_the_engine_parser() {
        let nodes = "id = \"x\"\nname = \"x\"\ndescription = \"d\"\nversion = \"1.0.0\"\n[[nodes]]\nid = \"n\"\nname = \"n\"\ntool = \"t\"\n";
        let err = converter().from_toml(nodes).await.unwrap_err();
        assert!(format!("{err}").contains("schema_version"), "{err}");
    }

    #[tokio::test]
    async fn an_engine_action_form_is_not_a_plan_step() {
        let nodes = "schema_version = 1\nid = \"x\"\nname = \"x\"\ndescription = \"d\"\nversion = \"1.0.0\"\n[[nodes]]\nid = \"n\"\nname = \"n\"\n[nodes.action]\ntype = \"run_command\"\ncommand = \"ls\"\n";
        let err = converter().from_toml(nodes).await.unwrap_err();
        let message = format!("{err}");
        assert!(message.contains("run_command"), "{message}");
        assert!(message.contains("not a plan step"), "{message}");
    }

    #[tokio::test]
    async fn to_toml_writes_the_engine_schema_and_round_trips() {
        let template = converter().from_toml(DEMO).await.expect("parses");
        let written = converter().to_toml(&template).await.expect("writes");
        assert!(
            written.starts_with("schema_version = 1"),
            "the version is the first field: {written}"
        );
        let again = converter().from_toml(&written).await.expect("reads back");
        assert_eq!(again.name(), template.name());
        assert_eq!(again.steps()[0].tool(), template.steps()[0].tool());
    }

    #[tokio::test]
    async fn the_converter_is_send_sync() {
        let shared: std::sync::Arc<dyn TemplateConverter> =
            std::sync::Arc::new(FilesystemTemplateConverter::new());
        let _ = shared;
    }
}
