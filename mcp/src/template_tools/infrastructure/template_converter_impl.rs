//! FilesystemTemplateConverter — Concrete implementation of TemplateConverter.
//!
//! @canonical .pi/architecture/modules/template-tools.md#template-converter
//! Implements: TemplateConverter contract — TOML ↔ JSON conversion with validation
//!
//! Converts PlanTemplate between TOML (filesystem storage) and JSON (MCP transport).
//! All conversion paths include schema validation.

use async_trait::async_trait;

use crate::template_tools::domain::entity::TemplateConverter;
use crate::template_tools::domain::error::TemplateError;
use crate::template_tools::domain::value::PlanTemplate;

/// Converts PlanTemplate between TOML and JSON formats.
///
/// TOML is used for filesystem storage (.rigorix/templates/*.toml).
/// JSON is used for MCP transport.
///
/// # Contract (Frozen)
///
/// - All methods match TemplateConverter trait exactly
/// - All errors are wrapped in TemplateError variants
/// - Thread-safe (Send + Sync)
pub struct FilesystemTemplateConverter;

impl FilesystemTemplateConverter {
    /// Create a new FilesystemTemplateConverter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for FilesystemTemplateConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TemplateConverter for FilesystemTemplateConverter {
    async fn to_toml(&self, template: &PlanTemplate) -> Result<String, TemplateError> {
        let json_value = serde_json::to_value(template).map_err(|e| {
            TemplateError::SerializationFailed(format!(
                "Failed to serialize template '{}' to JSON: {}",
                template.name(),
                e
            ))
        })?;

        let toml_value: toml::Value = serde_json::from_value(json_value).map_err(|e| {
            TemplateError::SerializationFailed(format!(
                "Failed to convert template '{}' JSON to TOML: {}",
                template.name(),
                e
            ))
        })?;

        toml::to_string(&toml_value).map_err(|e| {
            TemplateError::SerializationFailed(format!(
                "Failed to format template '{}' as TOML string: {}",
                template.name(),
                e
            ))
        })
    }

    async fn to_json(&self, template: &PlanTemplate) -> Result<serde_json::Value, TemplateError> {
        serde_json::to_value(template).map_err(|e| {
            TemplateError::SerializationFailed(format!(
                "Failed to serialize template '{}' to JSON: {}",
                template.name(),
                e
            ))
        })
    }

    async fn from_toml(&self, toml_str: &str) -> Result<PlanTemplate, TemplateError> {
        let toml_value: toml::Value = toml::from_str(toml_str).map_err(|e| {
            TemplateError::DeserializationFailed(format!("Failed to parse TOML: {}", e))
        })?;

        let json_value = serde_json::to_value(&toml_value).map_err(|e| {
            TemplateError::DeserializationFailed(format!("Failed to convert TOML to JSON: {}", e))
        })?;

        PlanTemplate::from_json(json_value)
    }

    async fn from_json(&self, json: serde_json::Value) -> Result<PlanTemplate, TemplateError> {
        PlanTemplate::from_json(json)
    }

    async fn validate_toml(&self, toml_str: &str) -> Result<(), TemplateError> {
        // Parse TOML to verify syntax
        let toml_value: toml::Value = toml::from_str(toml_str).map_err(|e| {
            TemplateError::DeserializationFailed(format!("Invalid TOML syntax: {}", e))
        })?;

        // Convert to JSON and validate as PlanTemplate
        let json_value = serde_json::to_value(&toml_value).map_err(|e| {
            TemplateError::DeserializationFailed(format!(
                "Failed to convert TOML to JSON for validation: {}",
                e
            ))
        })?;

        PlanTemplate::from_json(json_value)?;

        Ok(())
    }

    async fn validate_and_convert(
        &self,
        json: serde_json::Value,
    ) -> Result<PlanTemplate, TemplateError> {
        PlanTemplate::from_json(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_tools::domain::value::StepDefinition;
    use std::collections::HashMap;

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
            vec!["tag1".into()],
            vec![step],
            None,
            HashMap::new(),
            now,
            now,
        )
        .expect("Failed to create test template")
    }

    #[tokio::test]
    async fn test_convert_to_json() {
        let converter = FilesystemTemplateConverter::new();
        let template = test_template("json-test");
        let json = converter.to_json(&template).await.expect("Should convert");
        assert_eq!(json["name"], "json-test");
        assert_eq!(json["version"], "1.0.0");
        assert_eq!(json["tags"][0], "tag1");
    }

    #[tokio::test]
    async fn test_convert_to_toml() {
        let converter = FilesystemTemplateConverter::new();
        let template = test_template("toml-test");
        let toml = converter.to_toml(&template).await.expect("Should convert");
        assert!(toml.contains("toml-test"), "TOML should contain the name");
        assert!(toml.contains("1.0.0"), "TOML should contain version");
    }

    #[tokio::test]
    async fn test_toml_to_template_and_back() {
        let converter = FilesystemTemplateConverter::new();
        let original = test_template("roundtrip");

        // Convert to TOML
        let toml = converter
            .to_toml(&original)
            .await
            .expect("Should convert to TOML");

        // Parse back
        let parsed = converter.from_toml(&toml).await.expect("Should parse TOML");
        assert_eq!(parsed.name(), "roundtrip");
        assert_eq!(parsed.steps().len(), 1);
    }

    #[tokio::test]
    async fn test_json_to_template_and_back() {
        let converter = FilesystemTemplateConverter::new();
        let original = test_template("json-roundtrip");

        // Convert to JSON
        let json = converter
            .to_json(&original)
            .await
            .expect("Should convert to JSON");

        // Parse back as JSON value
        let parsed = converter.from_json(json).await.expect("Should parse JSON");
        assert_eq!(parsed.name(), "json-roundtrip");
        assert_eq!(parsed.steps().len(), 1);
    }

    #[tokio::test]
    async fn test_validate_and_convert_valid() {
        let converter = FilesystemTemplateConverter::new();
        let json = serde_json::json!({
            "name": "valid-template",
            "description": "A valid template",
            "version": "1.0.0",
            "tags": [],
            "steps": [{
                "name": "step-1",
                "tool": "test_tool",
                "parameters": {},
                "requires_approval": false,
                "description": "Test step"
            }],
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });

        let template = converter
            .validate_and_convert(json)
            .await
            .expect("Should validate");
        assert_eq!(template.name(), "valid-template");
    }

    #[tokio::test]
    async fn test_validate_and_convert_invalid_empty_steps() {
        let converter = FilesystemTemplateConverter::new();
        let json = serde_json::json!({
            "name": "invalid",
            "description": "No steps",
            "version": "1.0.0",
            "tags": [],
            "steps": [],
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });

        let err = converter.validate_and_convert(json).await.unwrap_err();
        assert!(
            matches!(err, TemplateError::ValidationError(_)),
            "Expected ValidationError for empty steps"
        );
    }

    #[tokio::test]
    async fn test_validate_toml_valid() {
        let converter = FilesystemTemplateConverter::new();
        let template = test_template("toml-validate");
        let toml = converter.to_toml(&template).await.expect("Should convert");

        let result = converter.validate_toml(&toml).await;
        assert!(result.is_ok(), "Valid TOML should pass validation");
    }

    #[tokio::test]
    async fn test_validate_toml_invalid_syntax() {
        let converter = FilesystemTemplateConverter::new();
        let invalid_toml = "name = 'unclosed string";

        let err = converter.validate_toml(invalid_toml).await.unwrap_err();
        assert!(
            matches!(err, TemplateError::DeserializationFailed(_)),
            "Expected DeserializationFailed for invalid TOML"
        );
    }

    #[tokio::test]
    async fn test_from_json_valid() {
        let converter = FilesystemTemplateConverter::new();
        let json = serde_json::json!({
            "name": "from-json",
            "description": "test",
            "version": "1.0.0",
            "tags": [],
            "steps": [{
                "name": "step-1",
                "tool": "tool",
                "parameters": {},
                "requires_approval": false,
                "description": "test"
            }],
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });

        let template = converter.from_json(json).await.expect("Should parse");
        assert_eq!(template.name(), "from-json");
    }

    #[tokio::test]
    async fn test_from_json_empty_steps() {
        let converter = FilesystemTemplateConverter::new();
        let json = serde_json::json!({
            "name": "invalid",
            "description": "test",
            "version": "1.0.0",
            "tags": [],
            "steps": [],
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });

        let err = converter.from_json(json).await.unwrap_err();
        assert!(
            matches!(err, TemplateError::ValidationError(_)),
            "Expected ValidationError"
        );
    }

    #[tokio::test]
    async fn test_converter_is_send_sync() {
        let converter = FilesystemTemplateConverter::new();
        let shared: std::sync::Arc<dyn TemplateConverter> = std::sync::Arc::new(converter);
        let _ = shared;
    }
}
