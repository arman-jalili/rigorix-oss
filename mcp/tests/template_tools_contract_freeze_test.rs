//! Contract Freeze validation tests for template-tools.
//!
//! These tests verify that all frozen contracts compile and are usable:
//! module structure, domain types, traits, DTOs, error types, events,
//! and MCP tool schemas.
//!
//! Actual implementation should be tested via mock-based unit tests;
//! this file validates the contracts themselves.
//!
//! @canonical .pi/architecture/modules/template-tools.md
//! Implements: Contract Freeze — template-tools module (issue #653)

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use rigorix_mcp::template_tools::application::dto::{
    CreateTemplateOutput, GetTemplateOutput, ListTemplatesOutput, TemplateSummaryDto,
    ValidateTemplateOutput,
};
use rigorix_mcp::template_tools::application::service::{
    CreateTemplateHandler, GetTemplateHandler, ListTemplatesHandler, ValidateTemplateHandler,
};
use rigorix_mcp::template_tools::domain::entity::{TemplateConverter, TemplateRepository};
use rigorix_mcp::template_tools::domain::error::{HandlerError, TemplateError, ToolCallResult};
use rigorix_mcp::template_tools::domain::event::TemplateToolsEvent;
use rigorix_mcp::template_tools::domain::value::{
    Constraints, CreateTemplateInput, GetTemplateInput, PlanTemplate, StepDefinition,
    TemplateFilter, TemplateName, TemplateSummary, ValidateTemplateInput,
};
use rigorix_mcp::template_tools::infrastructure::repository::TemplateRepositoryConfig;
use rigorix_mcp::template_tools::interfaces::mcp::{
    TEMPLATE_TOOL_NAMES, example_create_template_output, example_get_template_output,
    example_list_templates_output, example_validate_template_output,
    rigorix_create_template_tool_descriptor, rigorix_get_template_tool_descriptor,
    rigorix_list_templates_tool_descriptor, rigorix_validate_template_tool_descriptor,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a minimal valid PlanTemplate for testing.
fn make_test_template(name: &str) -> PlanTemplate {
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
        HashMap::new(),
        now,
        now,
    )
    .expect("Failed to create test template")
}

// ---------------------------------------------------------------------------
// Module structure — all layers exist and are accessible
// ---------------------------------------------------------------------------

#[test]
fn test_module_structure_accessible() {
    // Verify all four layers are accessible
    let _domain = ();
    let _application = ();
    let _infrastructure = ();
    let _interfaces = ();
}

// ---------------------------------------------------------------------------
// Domain layer — value objects
// ---------------------------------------------------------------------------

#[test]
fn test_template_name_valid() {
    let name = TemplateName::new("my-template").expect("Valid name should succeed");
    assert_eq!(name.as_str(), "my-template");
    assert_eq!(name.to_string(), "my-template");
}

#[test]
fn test_template_name_empty() {
    let err = TemplateName::new("").unwrap_err();
    assert!(matches!(err, TemplateError::InvalidName(_)));
}

#[test]
fn test_template_name_invalid_chars() {
    let err = TemplateName::new("test template!").unwrap_err();
    assert!(matches!(err, TemplateError::InvalidName(_)));
}

#[test]
fn test_template_name_too_long() {
    let long = "a".repeat(256);
    let err = TemplateName::new(long).unwrap_err();
    assert!(matches!(err, TemplateError::InvalidName(_)));
}

#[test]
fn test_plan_template_creation_with_steps() {
    let template = make_test_template("test-plan");
    assert_eq!(template.name(), "test-plan");
    assert_eq!(template.steps().len(), 1);
    assert_eq!(template.version(), "1.0.0");
}

#[test]
fn test_plan_template_empty_steps_fails() {
    let now = chrono::Utc::now();
    let err = PlanTemplate::new(
        "empty".into(),
        "desc".into(),
        "1.0.0".into(),
        vec![],
        vec![],
        None,
        HashMap::new(),
        now,
        now,
    )
    .unwrap_err();
    assert!(matches!(err, TemplateError::ValidationError(_)));
}

#[test]
fn test_plan_template_from_json_valid() {
    let json = serde_json::json!({
        "name": "json-test",
        "description": "Created from JSON",
        "version": "1.0.0",
        "tags": ["test"],
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
    let template = PlanTemplate::from_json(json).expect("Valid JSON should deserialize");
    assert_eq!(template.name(), "json-test");
    assert_eq!(template.steps().len(), 1);
}

#[test]
fn test_template_summary_creation() {
    let now = chrono::Utc::now();
    let summary = TemplateSummary::new(
        "test".into(),
        "desc".into(),
        "1.0.0".into(),
        vec!["tag1".into()],
        3,
        now,
    );
    assert_eq!(summary.name(), "test");
    assert_eq!(summary.step_count(), 3);
    assert_eq!(summary.tags(), &["tag1"]);
}

#[test]
fn test_template_filter_default() {
    let filter = TemplateFilter::default();
    assert_eq!(filter.limit(), 50);
    assert!(filter.tags().is_none());
    assert!(filter.search().is_none());
}

#[test]
fn test_template_filter_with_values() {
    let filter = TemplateFilter::new(
        Some(vec!["rust".into(), "test".into()]),
        Some("search-term".into()),
        Some(10),
    );
    assert_eq!(filter.limit(), 10);
    assert_eq!(filter.tags().unwrap(), &["rust", "test"]);
    assert_eq!(filter.search().unwrap(), "search-term");
}

#[test]
fn test_step_definition_creation() {
    let step = StepDefinition::new(
        "step-1".into(),
        "test_tool".into(),
        serde_json::json!({"key": "value"}),
        true,
        "Test step".into(),
        Some(30),
    );
    assert_eq!(step.name(), "step-1");
    assert_eq!(step.tool(), "test_tool");
    assert!(step.requires_approval());
    assert_eq!(step.timeout_secs(), Some(30));
}

#[test]
fn test_constraints_struct() {
    let constraints = Constraints {
        max_tool_calls: Some(100),
        max_tokens: Some(10000),
        max_duration_secs: Some(3600),
        blocked_tools: vec!["dangerous_tool".into()],
        extensions: HashMap::new(),
    };
    assert_eq!(constraints.max_tool_calls, Some(100));
    assert_eq!(constraints.max_tokens, Some(10000));
    assert_eq!(constraints.max_duration_secs, Some(3600));
}

#[test]
fn test_get_template_input() {
    let input = GetTemplateInput {
        name: "my-template".into(),
        format: Some("toml".into()),
    };
    assert_eq!(input.name, "my-template");
    assert_eq!(input.format, Some("toml".into()));
}

#[test]
fn test_get_template_input_default_format() {
    let input = GetTemplateInput {
        name: "my-template".into(),
        format: None,
    };
    assert!(input.format.is_none());
}

#[test]
fn test_create_template_input() {
    let input = CreateTemplateInput {
        name: "new-template".into(),
        plan: make_test_template("new-template"),
        overwrite: false,
    };
    assert_eq!(input.name, "new-template");
    assert!(!input.overwrite);
}

#[test]
fn test_create_template_input_with_overwrite() {
    let input = CreateTemplateInput {
        name: "overwrite-template".into(),
        plan: make_test_template("overwrite-template"),
        overwrite: true,
    };
    assert!(input.overwrite);
}

#[test]
fn test_validate_template_input() {
    let input = ValidateTemplateInput {
        plan: make_test_template("validate-me"),
    };
    assert_eq!(input.plan.name(), "validate-me");
}

// ---------------------------------------------------------------------------
// Domain layer — error types
// ---------------------------------------------------------------------------

#[test]
fn test_template_error_not_found() {
    let err = TemplateError::NotFound("test".into());
    assert_eq!(err.to_string(), "Template not found: test");
}

#[test]
fn test_template_error_already_exists() {
    let err = TemplateError::AlreadyExists("dup".into());
    assert_eq!(err.to_string(), "Template already exists: dup");
}

#[test]
fn test_template_error_validation() {
    let err = TemplateError::ValidationError("Invalid structure".into());
    assert!(err.to_string().contains("Invalid structure"));
}

#[test]
fn test_handler_error_invalid_arguments() {
    let err = HandlerError::InvalidArguments("name is required".into());
    assert_eq!(err.to_string(), "Invalid arguments: name is required");
}

#[test]
fn test_handler_error_from_template_error() {
    let template_err = TemplateError::NotFound("test".into());
    let handler_err: HandlerError = template_err.into();
    assert!(handler_err.to_string().contains("Template not found"));
}

#[test]
fn test_tool_call_result_success() {
    let result = ToolCallResult::success(serde_json::json!({"key": "value"}));
    assert!(!result.is_error);
    assert!(!result.content.is_empty());
    assert_eq!(result.content[0].r#type, "text");
}

#[test]
fn test_tool_call_result_error() {
    let result = ToolCallResult::error("Something went wrong");
    assert!(result.is_error);
    assert!(!result.content.is_empty());
    assert!(result.content[0].text.contains("Something went wrong"));
}

// ---------------------------------------------------------------------------
// Domain layer — events
// ---------------------------------------------------------------------------

#[test]
fn test_template_created_event() {
    let event = TemplateToolsEvent::TemplateCreated {
        template_name: "new-template".into(),
        step_count: 3,
        overwrite: false,
        timestamp: chrono::Utc::now(),
    };
    let json = serde_json::to_value(&event).expect("Should serialize");
    assert_eq!(json["type"], "template_created");
    assert_eq!(json["template_name"], "new-template");
    assert_eq!(json["step_count"], 3);
}

#[test]
fn test_template_read_event() {
    let event = TemplateToolsEvent::TemplateRead {
        template_name: "my-template".into(),
        format: Some("json".into()),
        timestamp: chrono::Utc::now(),
    };
    let json = serde_json::to_value(&event).expect("Should serialize");
    assert_eq!(json["type"], "template_read");
    assert_eq!(json["template_name"], "my-template");
    assert_eq!(json["format"], "json");
}

#[test]
fn test_template_listed_event() {
    let event = TemplateToolsEvent::TemplateListed {
        filter_criteria: Some("tags=demo".into()),
        result_count: 5,
        timestamp: chrono::Utc::now(),
    };
    let json = serde_json::to_value(&event).expect("Should serialize");
    assert_eq!(json["type"], "template_listed");
    assert_eq!(json["result_count"], 5);
}

#[test]
fn test_template_validated_event() {
    let event = TemplateToolsEvent::TemplateValidated {
        template_name: "my-template".into(),
        is_valid: true,
        errors: vec![],
        timestamp: chrono::Utc::now(),
    };
    let json = serde_json::to_value(&event).expect("Should serialize");
    assert_eq!(json["type"], "template_validated");
    assert!(json["is_valid"].as_bool().unwrap());
}

// ---------------------------------------------------------------------------
// Domain layer — entity traits (TemplateRepository + TemplateConverter)
// ---------------------------------------------------------------------------

/// Adapter to make TemplateRepository mut methods work with async_trait.
struct MockTemplateRepository {
    templates: std::sync::Mutex<std::collections::HashMap<String, PlanTemplate>>,
}

impl MockTemplateRepository {
    fn new() -> Self {
        Self {
            templates: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl TemplateRepository for MockTemplateRepository {
    async fn list(&self, _filter: &TemplateFilter) -> Result<Vec<TemplateSummary>, TemplateError> {
        let templates = self.templates.lock().unwrap();
        Ok(templates
            .values()
            .map(|t| {
                TemplateSummary::new(
                    t.name().to_string(),
                    t.description().to_string(),
                    t.version().to_string(),
                    t.tags().to_vec(),
                    t.steps().len(),
                    *t.updated_at(),
                )
            })
            .collect())
    }

    async fn get(&self, name: &str) -> Result<PlanTemplate, TemplateError> {
        let templates = self.templates.lock().unwrap();
        templates
            .get(name)
            .cloned()
            .ok_or_else(|| TemplateError::NotFound(name.to_string()))
    }

    async fn create(&self, template: PlanTemplate, overwrite: bool) -> Result<(), TemplateError> {
        let mut templates = self.templates.lock().unwrap();
        let name = template.name().to_string();
        if !overwrite && templates.contains_key(&name) {
            return Err(TemplateError::AlreadyExists(name));
        }
        templates.insert(name, template);
        Ok(())
    }

    async fn delete(&self, name: &str) -> Result<(), TemplateError> {
        let mut templates = self.templates.lock().unwrap();
        if templates.remove(name).is_none() {
            return Err(TemplateError::NotFound(name.to_string()));
        }
        Ok(())
    }

    async fn exists(&self, name: &str) -> Result<bool, TemplateError> {
        let templates = self.templates.lock().unwrap();
        Ok(templates.contains_key(name))
    }
}

#[tokio::test]
async fn test_template_repository_trait_impl() {
    let repo = MockTemplateRepository::new();
    let template = make_test_template("test");

    repo.create(template.clone(), false)
        .await
        .expect("Create should succeed");

    let retrieved = repo.get("test").await.expect("Get should succeed");
    assert_eq!(retrieved.name(), "test");
    assert!(repo.exists("test").await.unwrap());

    // Verify Send + Sync
    let arc: Arc<dyn TemplateRepository> = Arc::new(repo);
    let _ = arc.list(&TemplateFilter::default()).await;
}

#[tokio::test]
async fn test_template_repository_send_sync() {
    let repo: Arc<dyn TemplateRepository> = Arc::new(MockTemplateRepository::new());
    let filter = TemplateFilter::default();
    let _ = repo.list(&filter).await;
}

struct MockTemplateConverter;

#[async_trait]
impl TemplateConverter for MockTemplateConverter {
    async fn to_toml(&self, template: &PlanTemplate) -> Result<String, TemplateError> {
        Ok(format!("name = \"{}\"", template.name()))
    }

    async fn to_json(&self, template: &PlanTemplate) -> Result<serde_json::Value, TemplateError> {
        Ok(serde_json::json!({"name": template.name()}))
    }

    async fn from_toml(&self, _toml_str: &str) -> Result<PlanTemplate, TemplateError> {
        Ok(make_test_template("from-toml"))
    }

    async fn from_json(&self, json: serde_json::Value) -> Result<PlanTemplate, TemplateError> {
        PlanTemplate::from_json(json)
    }

    async fn validate_toml(&self, _toml_str: &str) -> Result<(), TemplateError> {
        Ok(())
    }

    async fn validate_and_convert(
        &self,
        json: serde_json::Value,
    ) -> Result<PlanTemplate, TemplateError> {
        PlanTemplate::from_json(json)
    }
}

#[tokio::test]
async fn test_template_converter_trait_impl() {
    let converter = MockTemplateConverter;
    let template = make_test_template("convert-test");
    let toml = converter.to_toml(&template).await.expect("TOML conversion");
    assert!(toml.contains("convert-test"));
}

#[tokio::test]
async fn test_template_converter_send_sync() {
    let converter: Arc<dyn TemplateConverter> = Arc::new(MockTemplateConverter);
    let _ = converter;
}

// ---------------------------------------------------------------------------
// Application layer — service traits
// ---------------------------------------------------------------------------

struct MockListHandler;

#[async_trait]
impl ListTemplatesHandler for MockListHandler {
    async fn handle(&self, _filter: &TemplateFilter) -> Result<ToolCallResult, HandlerError> {
        Ok(ToolCallResult::success(
            serde_json::json!({"templates": []}),
        ))
    }
}

#[tokio::test]
async fn test_list_templates_handler_trait() {
    let handler = MockListHandler;
    let filter = TemplateFilter::default();
    let result = handler.handle(&filter).await.expect("Should succeed");
    assert!(!result.is_error);
}

struct MockGetHandler;

#[async_trait]
impl GetTemplateHandler for MockGetHandler {
    async fn handle(&self, input: &GetTemplateInput) -> Result<ToolCallResult, HandlerError> {
        Ok(ToolCallResult::success(
            serde_json::json!({"name": input.name}),
        ))
    }
}

#[tokio::test]
async fn test_get_template_handler_trait() {
    let handler = MockGetHandler;
    let input = GetTemplateInput {
        name: "test".into(),
        format: None,
    };
    let result = handler.handle(&input).await.expect("Should succeed");
    let json: serde_json::Value =
        serde_json::from_str(&result.content[0].text).expect("Should be JSON");
    assert_eq!(json["name"], "test");
}

struct MockCreateHandler;

#[async_trait]
impl CreateTemplateHandler for MockCreateHandler {
    async fn handle(&self, input: &CreateTemplateInput) -> Result<ToolCallResult, HandlerError> {
        Ok(ToolCallResult::success(serde_json::json!({
            "name": input.name,
            "status": "created"
        })))
    }
}

#[tokio::test]
async fn test_create_template_handler_trait() {
    let handler = MockCreateHandler;
    let input = CreateTemplateInput {
        name: "new".into(),
        plan: make_test_template("new"),
        overwrite: false,
    };
    let result = handler.handle(&input).await.expect("Should succeed");
    let json: serde_json::Value =
        serde_json::from_str(&result.content[0].text).expect("Should be JSON");
    assert_eq!(json["name"], "new");
    assert_eq!(json["status"], "created");
}

struct MockValidateHandler;

#[async_trait]
impl ValidateTemplateHandler for MockValidateHandler {
    async fn handle(&self, _input: &ValidateTemplateInput) -> Result<ToolCallResult, HandlerError> {
        Ok(ToolCallResult::success(serde_json::json!({
            "valid": true,
            "warnings": [],
            "errors": []
        })))
    }
}

#[tokio::test]
async fn test_validate_template_handler_trait() {
    let handler = MockValidateHandler;
    let input = ValidateTemplateInput {
        plan: make_test_template("valid"),
    };
    let result = handler.handle(&input).await.expect("Should succeed");
    assert!(!result.is_error);
}

// ---------------------------------------------------------------------------
// Application layer — DTOs
// ---------------------------------------------------------------------------

#[test]
fn test_list_templates_output_dto() {
    let now = chrono::Utc::now();
    let output = ListTemplatesOutput {
        templates: vec![TemplateSummaryDto {
            name: "test".into(),
            description: "desc".into(),
            version: "1.0.0".into(),
            tags: vec![],
            step_count: 1,
            updated_at: now,
        }],
    };
    let json = serde_json::to_value(&output).expect("Should serialize");
    assert!(!json["templates"].as_array().unwrap().is_empty());
}

#[test]
fn test_get_template_output_dto() {
    let now = chrono::Utc::now();
    let output = GetTemplateOutput {
        name: "test".into(),
        description: "desc".into(),
        version: "1.0.0".into(),
        tags: vec![],
        steps: vec![],
        constraints: None,
        metadata: HashMap::new(),
        created_at: now,
        updated_at: now,
    };
    let json = serde_json::to_value(&output).expect("Should serialize");
    assert_eq!(json["name"], "test");
}

#[test]
fn test_create_template_output_dto() {
    let output = CreateTemplateOutput {
        name: "test".into(),
        path: ".rigorix/templates/test.toml".into(),
        status: "created".into(),
    };
    let json = serde_json::to_value(&output).expect("Should serialize");
    assert_eq!(json["status"], "created");
}

#[test]
fn test_validate_template_output_dto() {
    let output = ValidateTemplateOutput {
        valid: true,
        warnings: vec![],
        errors: vec![],
        estimated_cost: None,
    };
    let json = serde_json::to_value(&output).expect("Should serialize");
    assert!(json["valid"].as_bool().unwrap());
}

// ---------------------------------------------------------------------------
// Infrastructure layer
// ---------------------------------------------------------------------------

#[test]
fn test_template_repository_config_default() {
    let config = TemplateRepositoryConfig::default_path();
    assert_eq!(config.base_path().to_str().unwrap(), ".rigorix/templates");
}

#[test]
fn test_template_repository_config_custom() {
    let config = TemplateRepositoryConfig::new("/tmp/test-templates");
    assert_eq!(config.base_path().to_str().unwrap(), "/tmp/test-templates");
}

// ---------------------------------------------------------------------------
// Interfaces layer — MCP tool schemas
// ---------------------------------------------------------------------------

#[test]
fn test_tool_names_defined() {
    assert!(TEMPLATE_TOOL_NAMES.contains(&"rigorix_list_templates"));
    assert!(TEMPLATE_TOOL_NAMES.contains(&"rigorix_get_template"));
    assert!(TEMPLATE_TOOL_NAMES.contains(&"rigorix_create_template"));
    assert!(TEMPLATE_TOOL_NAMES.contains(&"rigorix_validate_template"));
    assert_eq!(TEMPLATE_TOOL_NAMES.len(), 4);
}

#[test]
fn test_list_templates_schema() {
    let descriptor = rigorix_list_templates_tool_descriptor();
    assert_eq!(descriptor["name"], "rigorix_list_templates");
    assert!(descriptor["inputSchema"].is_object());
}

#[test]
fn test_get_template_schema() {
    let descriptor = rigorix_get_template_tool_descriptor();
    assert_eq!(descriptor["name"], "rigorix_get_template");
    assert!(descriptor["inputSchema"].is_object());
}

#[test]
fn test_create_template_schema() {
    let descriptor = rigorix_create_template_tool_descriptor();
    assert_eq!(descriptor["name"], "rigorix_create_template");
    assert!(descriptor["inputSchema"].is_object());
}

#[test]
fn test_validate_template_schema() {
    let descriptor = rigorix_validate_template_tool_descriptor();
    assert_eq!(descriptor["name"], "rigorix_validate_template");
    assert!(descriptor["inputSchema"].is_object());
}

#[test]
fn test_list_templates_input_schema_has_properties() {
    let schema: serde_json::Value = serde_json::from_str(
        rigorix_mcp::template_tools::interfaces::mcp::RIGORIX_LIST_TEMPLATES_INPUT_SCHEMA,
    )
    .expect("Valid JSON schema");
    assert!(schema["properties"]["tags"].is_object());
    assert!(schema["properties"]["search"].is_object());
    assert!(schema["properties"]["limit"].is_object());
}

#[test]
fn test_get_template_input_schema_has_required_name() {
    let schema: serde_json::Value = serde_json::from_str(
        rigorix_mcp::template_tools::interfaces::mcp::RIGORIX_GET_TEMPLATE_INPUT_SCHEMA,
    )
    .expect("Valid JSON schema");
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "name"));
}

#[test]
fn test_create_template_input_schema_has_required_fields() {
    let schema: serde_json::Value = serde_json::from_str(
        rigorix_mcp::template_tools::interfaces::mcp::RIGORIX_CREATE_TEMPLATE_INPUT_SCHEMA,
    )
    .expect("Valid JSON schema");
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "name"));
    assert!(required.iter().any(|v| v == "plan"));
}

#[test]
fn test_validate_template_input_schema_has_required_plan() {
    let schema: serde_json::Value = serde_json::from_str(
        rigorix_mcp::template_tools::interfaces::mcp::RIGORIX_VALIDATE_TEMPLATE_INPUT_SCHEMA,
    )
    .expect("Valid JSON schema");
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "plan"));
}

// ---------------------------------------------------------------------------
// Example outputs
// ---------------------------------------------------------------------------

#[test]
fn test_example_list_templates_output() {
    let output = example_list_templates_output();
    assert!(!output.templates.is_empty());
    assert_eq!(output.templates[0].name, "example-template");
}

#[test]
fn test_example_get_template_output() {
    let output = example_get_template_output();
    assert_eq!(output.name, "example-template");
}

#[test]
fn test_example_create_template_output() {
    let output = example_create_template_output();
    assert_eq!(output.name, "example-template");
    assert_eq!(output.status, "created");
}

#[test]
fn test_example_validate_template_output() {
    let output = example_validate_template_output();
    assert!(output.valid);
}
