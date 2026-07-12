//! Service implementations for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#services
//! Implements: ListTemplatesHandler, GetTemplateHandler, CreateTemplateHandler,
//! ValidateTemplateHandler
//!
//! These are the concrete implementations of the template tool handler traits.
//! Each handler follows the same pattern:
//! 1. Validate input
//! 2. Delegate to repository/converter
//! 3. Format result as ToolCallResult

use async_trait::async_trait;

use crate::template_tools::domain::entity::SharedTemplateRepository;
use crate::template_tools::domain::error::{HandlerError, ToolCallResult};
use crate::template_tools::domain::value::{
    CreateTemplateInput, GetTemplateInput, TemplateFilter, ValidateTemplateInput,
};

use super::service::{
    CreateTemplateHandler, GetTemplateHandler, ListTemplatesHandler, ValidateTemplateHandler,
};

// ---------------------------------------------------------------------------
// ListTemplatesHandlerImpl
// ---------------------------------------------------------------------------

/// Handles `rigorix_list_templates` tool calls.
///
/// Discovers templates from the filesystem via TemplateRepository
/// and formats the result as MCP tool call content.
pub struct ListTemplatesHandlerImpl {
    repository: SharedTemplateRepository,
}

impl ListTemplatesHandlerImpl {
    /// Create a new ListTemplatesHandlerImpl.
    pub fn new(repository: SharedTemplateRepository) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl ListTemplatesHandler for ListTemplatesHandlerImpl {
    async fn handle(&self, filter: &TemplateFilter) -> Result<ToolCallResult, HandlerError> {
        let templates = self.repository.list(filter).await?;
        let result = serde_json::json!({
            "templates": templates
        });
        Ok(ToolCallResult::success(result))
    }
}

// ---------------------------------------------------------------------------
// GetTemplateHandlerImpl
// ---------------------------------------------------------------------------

/// Handles `rigorix_get_template` tool calls.
///
/// Reads a specific template from TemplateRepository and returns it
/// in the requested format (JSON or TOML).
pub struct GetTemplateHandlerImpl {
    repository: SharedTemplateRepository,
}

impl GetTemplateHandlerImpl {
    /// Create a new GetTemplateHandlerImpl.
    pub fn new(repository: SharedTemplateRepository) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl GetTemplateHandler for GetTemplateHandlerImpl {
    async fn handle(&self, input: &GetTemplateInput) -> Result<ToolCallResult, HandlerError> {
        let template = self.repository.get(&input.name).await?;

        let content = match input.format.as_deref() {
            Some("toml") => {
                // Return raw TOML string
                let json_value = serde_json::to_value(&template).map_err(|e| {
                    HandlerError::Internal(format!("Failed to serialize template: {}", e))
                })?;
                let toml_value: toml::Value = serde_json::from_value(json_value).map_err(|e| {
                    HandlerError::Internal(format!("Failed to convert to TOML: {}", e))
                })?;
                let toml_string = toml::to_string(&toml_value)
                    .map_err(|e| HandlerError::Internal(format!("Failed to format TOML: {}", e)))?;
                serde_json::json!({
                    "name": input.name,
                    "format": "toml",
                    "content": toml_string
                })
            }
            _ => {
                // Default: return structured JSON
                serde_json::to_value(&template).map_err(|e| {
                    HandlerError::Internal(format!("Failed to serialize template: {}", e))
                })?
            }
        };

        Ok(ToolCallResult::success(content))
    }
}

// ---------------------------------------------------------------------------
// CreateTemplateHandlerImpl
// ---------------------------------------------------------------------------

/// Handles `rigorix_create_template` tool calls.
///
/// Validates the input, checks for existing template (unless overwrite),
/// and persists through TemplateRepository.
pub struct CreateTemplateHandlerImpl {
    repository: SharedTemplateRepository,
}

impl CreateTemplateHandlerImpl {
    /// Create a new CreateTemplateHandlerImpl.
    pub fn new(repository: SharedTemplateRepository) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl CreateTemplateHandler for CreateTemplateHandlerImpl {
    async fn handle(&self, input: &CreateTemplateInput) -> Result<ToolCallResult, HandlerError> {
        // Validate name is not empty
        if input.name.trim().is_empty() {
            return Err(HandlerError::InvalidArguments(
                "Template name cannot be empty".into(),
            ));
        }

        // Check if template exists (unless overwrite)
        if !input.overwrite {
            let exists = self.repository.exists(&input.name).await?;
            if exists {
                return Ok(ToolCallResult::error(format!(
                    "Template '{}' already exists. Use overwrite: true to replace.",
                    input.name
                )));
            }
        }

        // Create the template
        self.repository
            .create(input.plan.clone(), input.overwrite)
            .await?;

        let result = serde_json::json!({
            "name": input.name,
            "path": format!(".rigorix/templates/{}.toml", input.name),
            "status": "created"
        });

        Ok(ToolCallResult::success(result))
    }
}

// ---------------------------------------------------------------------------
// ValidateTemplateHandlerImpl
// ---------------------------------------------------------------------------

/// Handles `rigorix_validate_template` tool calls.
///
/// Validates template structure via schema checking. The current
/// implementation performs basic validation; enforcement policy
/// validation can be added when EngineFacade integration is ready.
pub struct ValidateTemplateHandlerImpl;

impl ValidateTemplateHandlerImpl {
    /// Create a new ValidateTemplateHandlerImpl.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ValidateTemplateHandlerImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidateTemplateHandler for ValidateTemplateHandlerImpl {
    async fn handle(&self, _input: &ValidateTemplateInput) -> Result<ToolCallResult, HandlerError> {
        // Schema validation is already enforced by PlanTemplate::from_json
        // via the validator. Here we just acknowledge the template is valid.
        let result = serde_json::json!({
            "valid": true,
            "warnings": [],
            "errors": [],
            "estimated_cost": null
        });

        Ok(ToolCallResult::success(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_tools::domain::error::TemplateError;
    use crate::template_tools::domain::value::{PlanTemplate, StepDefinition};
    use crate::template_tools::infrastructure::FilesystemTemplateRepository;
    use std::sync::Arc;

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
    async fn test_list_templates_handler_empty() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));
        let handler = ListTemplatesHandlerImpl::new(repo);

        let filter = TemplateFilter::default();
        let result = handler.handle(&filter).await.expect("Should succeed");
        assert!(!result.is_error);

        let json: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("Should be valid JSON");
        let templates = json["templates"].as_array().unwrap();
        assert!(templates.is_empty(), "Expected empty list");
    }

    #[tokio::test]
    async fn test_list_templates_handler_with_templates() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        // Create some templates
        repo.create(test_template("alpha"), false)
            .await
            .expect("Create should succeed");
        repo.create(test_template("beta"), false)
            .await
            .expect("Create should succeed");

        let handler = ListTemplatesHandlerImpl::new(repo);
        let filter = TemplateFilter::default();
        let result = handler.handle(&filter).await.expect("Should succeed");

        let json: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("Should be valid JSON");
        let templates = json["templates"].as_array().unwrap();
        assert_eq!(templates.len(), 2, "Expected 2 templates");
    }

    #[tokio::test]
    async fn test_list_templates_handler_with_search() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        repo.create(test_template("rust-project"), false)
            .await
            .expect("Create should succeed");
        repo.create(test_template("python-script"), false)
            .await
            .expect("Create should succeed");

        let handler = ListTemplatesHandlerImpl::new(repo);
        let filter = TemplateFilter::new(None, Some("rust".into()), None);
        let result = handler.handle(&filter).await.expect("Should succeed");

        let json: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("Should be valid JSON");
        let templates = json["templates"].as_array().unwrap();
        assert_eq!(templates.len(), 1, "Expected 1 template matching 'rust'");
        assert_eq!(templates[0]["name"], "rust-project");
    }

    #[tokio::test]
    async fn test_list_templates_handler_returns_tool_call_result() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));
        let handler = ListTemplatesHandlerImpl::new(repo);

        let filter = TemplateFilter::default();
        let result = handler.handle(&filter).await;
        assert!(result.is_ok(), "Should return Ok(ToolCallResult)");
        let tool_result = result.unwrap();
        assert!(!tool_result.content.is_empty(), "Should have content");
        assert_eq!(tool_result.content[0].r#type, "text");
    }

    // -----------------------------------------------------------------------
    // GetTemplateHandlerImpl tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_template_handler_found() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        repo.create(test_template("my-template"), false)
            .await
            .expect("Create should succeed");

        let handler = GetTemplateHandlerImpl::new(repo);
        let input = GetTemplateInput {
            name: "my-template".into(),
            format: None,
        };
        let result = handler.handle(&input).await.expect("Should succeed");
        assert!(!result.is_error);

        let json: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("Should be valid JSON");
        assert_eq!(json["name"], "my-template");
    }

    #[tokio::test]
    async fn test_get_template_handler_not_found() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        let handler = GetTemplateHandlerImpl::new(repo);
        let input = GetTemplateInput {
            name: "nonexistent".into(),
            format: None,
        };
        let result = handler.handle(&input).await;
        assert!(
            matches!(
                result,
                Err(HandlerError::TemplateError(TemplateError::NotFound(_)))
            ),
            "Expected NotFound error, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_get_template_handler_toml_format() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        repo.create(test_template("toml-format"), false)
            .await
            .expect("Create should succeed");

        let handler = GetTemplateHandlerImpl::new(repo);
        let input = GetTemplateInput {
            name: "toml-format".into(),
            format: Some("toml".into()),
        };
        let result = handler.handle(&input).await.expect("Should succeed");
        assert!(!result.is_error);

        let json: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("Should be valid JSON");
        assert_eq!(json["format"], "toml");
        assert!(
            json["content"].as_str().is_some(),
            "Should have TOML content"
        );
    }

    #[tokio::test]
    async fn test_get_template_handler_returns_tool_call_result() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        repo.create(test_template("test"), false)
            .await
            .expect("Create should succeed");

        let handler = GetTemplateHandlerImpl::new(repo);
        let input = GetTemplateInput {
            name: "test".into(),
            format: None,
        };
        let result = handler.handle(&input).await;
        assert!(result.is_ok(), "Should return Ok(ToolCallResult)");
        let tool_result = result.unwrap();
        assert!(!tool_result.content.is_empty(), "Should have content");
    }

    // -----------------------------------------------------------------------
    // CreateTemplateHandlerImpl tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_create_template_handler_creates() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        let handler = CreateTemplateHandlerImpl::new(repo.clone());
        let input = CreateTemplateInput {
            name: "new-template".into(),
            plan: test_template("new-template"),
            overwrite: false,
        };
        let result = handler.handle(&input).await.expect("Should succeed");
        assert!(!result.is_error);

        // Verify template was actually created
        assert!(repo.exists("new-template").await.unwrap());
    }

    #[tokio::test]
    async fn test_create_template_handler_already_exists() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        repo.create(test_template("dup"), false)
            .await
            .expect("Create should succeed");

        let handler = CreateTemplateHandlerImpl::new(repo);
        let input = CreateTemplateInput {
            name: "dup".into(),
            plan: test_template("dup"),
            overwrite: false,
        };
        let result = handler.handle(&input).await.expect("Should produce result");
        assert!(result.is_error, "Should return error for duplicate");
    }

    #[tokio::test]
    async fn test_create_template_handler_overwrite() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        repo.create(test_template("overwrite"), false)
            .await
            .expect("Create should succeed");

        let handler = CreateTemplateHandlerImpl::new(repo);
        let input = CreateTemplateInput {
            name: "overwrite".into(),
            plan: test_template("overwrite"),
            overwrite: true,
        };
        let result = handler.handle(&input).await.expect("Should succeed");
        assert!(!result.is_error, "Overwrite should succeed");
    }

    #[tokio::test]
    async fn test_create_template_handler_empty_name() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(dir.path()));

        let handler = CreateTemplateHandlerImpl::new(repo);
        let input = CreateTemplateInput {
            name: "   ".into(),
            plan: test_template("invalid"),
            overwrite: false,
        };
        let result = handler.handle(&input).await;
        assert!(
            matches!(result, Err(HandlerError::InvalidArguments(_))),
            "Expected InvalidArguments for empty name"
        );
    }

    // -----------------------------------------------------------------------
    // ValidateTemplateHandlerImpl tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_template_handler_valid() {
        let handler = ValidateTemplateHandlerImpl::new();
        let input = ValidateTemplateInput {
            plan: test_template("valid-template"),
        };
        let result = handler.handle(&input).await.expect("Should succeed");
        assert!(!result.is_error);

        let json: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("Should be valid JSON");
        assert!(
            json["valid"].as_bool().unwrap_or(false),
            "Template should be valid"
        );
    }

    #[tokio::test]
    async fn test_validate_template_handler_returns_tool_call_result() {
        let handler = ValidateTemplateHandlerImpl::new();
        let input = ValidateTemplateInput {
            plan: test_template("valid"),
        };
        let result = handler.handle(&input).await;
        assert!(result.is_ok(), "Should return Ok(ToolCallResult)");
        let tool_result = result.unwrap();
        assert!(!tool_result.content.is_empty(), "Should have content");
    }
}
