//! MCP protocol handler contracts for Template Tools.
//!
//! @canonical .pi/architecture/modules/template-tools.md#mcp-handlers
//! Implements: Contract Freeze — rigorix_list_templates, rigorix_get_template,
//! rigorix_create_template, rigorix_validate_template tool handler contracts
//!
//! These contracts define the MCP tool schema registrations for the
//! template tools. Each handler defines:
//! - Tool name (as registered in ToolRegistry)
//! - Input JSON schema
//! - Output format
//! - Error conditions
//!
//! # Contract (Frozen)
//!
//! - Tool names are frozen (rigorix_list_templates, rigorix_get_template,
//!   rigorix_create_template, rigorix_validate_template)
//! - Input schemas are documented but not enforced by types here
//! - Output format follows MCP ToolResult specification
//! - Error format follows HandlerError type
//!
//! # API Endpoints
//!
//! | Method | Tool Name | Handler | Description |
//! |--------|-----------|---------|-------------|
//! | tools/call | `rigorix_list_templates` | ListTemplatesHandler | Discover templates from filesystem |
//! | tools/call | `rigorix_get_template` | GetTemplateHandler | Read a specific template by name |
//! | tools/call | `rigorix_create_template` | CreateTemplateHandler | Create a new template |
//! | tools/call | `rigorix_validate_template` | ValidateTemplateHandler | Validate template structure |

use serde_json::json;

use crate::template_tools::application::dto::{
    CreateTemplateOutput, GetTemplateOutput, ListTemplatesOutput, ValidateTemplateOutput,
};

// ---------------------------------------------------------------------------
// Tool Schema Definitions
// ---------------------------------------------------------------------------

/// JSON Schema for the `rigorix_list_templates` tool input.
pub const RIGORIX_LIST_TEMPLATES_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "tags": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Optional tag filter — only templates matching ALL specified tags"
        },
        "search": {
            "type": "string",
            "description": "Optional text search — matches against name and description"
        },
        "limit": {
            "type": "integer",
            "minimum": 1,
            "maximum": 100,
            "default": 50,
            "description": "Maximum number of results to return"
        }
    },
    "description": "List available plan templates with optional filtering"
}"#;

/// JSON Schema for the `rigorix_get_template` tool input.
pub const RIGORIX_GET_TEMPLATE_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "name": {
            "type": "string",
            "pattern": "^[a-zA-Z0-9_-]+$",
            "description": "Name of the template to retrieve (filesystem-safe characters only)"
        },
        "format": {
            "type": "string",
            "enum": ["json", "toml"],
            "default": "json",
            "description": "Output format — 'json' (default) for MCP transport or 'toml' for raw TOML content"
        }
    },
    "required": ["name"]
}"#;

/// JSON Schema for the `rigorix_create_template` tool input.
pub const RIGORIX_CREATE_TEMPLATE_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "name": {
            "type": "string",
            "pattern": "^[a-zA-Z0-9_-]+$",
            "description": "Name for the new template (filesystem-safe characters only)"
        },
        "plan": {
            "type": "object",
            "description": "The template plan to store",
            "properties": {
                "name": { "type": "string", "description": "Template name" },
                "description": { "type": "string", "description": "Template description" },
                "version": { "type": "string", "description": "Template version (semver)" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "steps": {
                    "type": "array",
                    "description": "Ordered list of steps",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "tool": { "type": "string" },
                            "parameters": { "type": "object" },
                            "requires_approval": { "type": "boolean" },
                            "description": { "type": "string" },
                            "timeout_secs": { "type": "integer" }
                        },
                        "required": ["name", "tool", "parameters"]
                    },
                    "minItems": 1
                },
                "constraints": {
                    "type": "object",
                    "description": "Optional enforcement constraints",
                    "properties": {
                        "max_tool_calls": { "type": "integer" },
                        "max_tokens": { "type": "integer" },
                        "max_duration_secs": { "type": "integer" }
                    }
                }
            },
            "required": ["name", "description", "steps"]
        },
        "overwrite": {
            "type": "boolean",
            "default": false,
            "description": "Whether to overwrite an existing template with the same name"
        }
    },
    "required": ["name", "plan"]
}"#;

/// JSON Schema for the `rigorix_validate_template` tool input.
pub const RIGORIX_VALIDATE_TEMPLATE_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "plan": {
            "type": "object",
            "description": "The template plan to validate against schema and enforcement policies",
            "properties": {
                "name": { "type": "string" },
                "description": { "type": "string" },
                "version": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "steps": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "tool": { "type": "string" },
                            "parameters": { "type": "object" },
                            "requires_approval": { "type": "boolean" },
                            "description": { "type": "string" },
                            "timeout_secs": { "type": "integer" }
                        },
                        "required": ["name", "tool", "parameters"]
                    },
                    "minItems": 1
                }
            },
            "required": ["name", "description", "steps"]
        }
    },
    "required": ["plan"]
}"#;

// ---------------------------------------------------------------------------
// MCP Tool Descriptors
// ---------------------------------------------------------------------------

/// Descriptor for the `rigorix_list_templates` tool.
///
/// Used for registering the tool schema in ToolRegistry.
pub fn rigorix_list_templates_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_list_templates",
        "description": "List available plan templates. Supports optional filtering by tags, text search, and result limit. Returns template summaries with name, description, version, tags, step count, and last update time.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_LIST_TEMPLATES_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

/// Descriptor for the `rigorix_get_template` tool.
pub fn rigorix_get_template_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_get_template",
        "description": "Get a specific plan template by name. Supports optional format selection: 'json' (default) for structured data or 'toml' for raw TOML content.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_GET_TEMPLATE_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

/// Descriptor for the `rigorix_create_template` tool.
pub fn rigorix_create_template_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_create_template",
        "description": "Create a new plan template. Validates the template structure, checks for name conflicts, and persists as a TOML file. Uses atomic write operations for data safety.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_CREATE_TEMPLATE_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

/// Descriptor for the `rigorix_validate_template` tool.
pub fn rigorix_validate_template_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_validate_template",
        "description": "Validate a plan template structure against the schema. Checks step definitions, constraints, and optionally validates against enforcement policies. Returns validation warnings, errors, and estimated cost.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_VALIDATE_TEMPLATE_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

// ---------------------------------------------------------------------------
// Example output formats
// ---------------------------------------------------------------------------

/// Example successful list templates output.
pub fn example_list_templates_output() -> ListTemplatesOutput {
    ListTemplatesOutput {
        templates: vec![
            crate::template_tools::application::dto::TemplateSummaryDto {
                name: "example-template".into(),
                description: "An example template showing the structure".into(),
                version: "1.0.0".into(),
                tags: vec!["example".into(), "demo".into()],
                step_count: 2,
                updated_at: chrono::Utc::now(),
            },
        ],
    }
}

/// Example successful get template output.
pub fn example_get_template_output() -> GetTemplateOutput {
    GetTemplateOutput {
        name: "example-template".into(),
        description: "An example template showing the structure".into(),
        version: "1.0.0".into(),
        tags: vec!["example".into()],
        steps: vec![],
        constraints: None,
        metadata: std::collections::HashMap::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Example successful create template output.
pub fn example_create_template_output() -> CreateTemplateOutput {
    CreateTemplateOutput {
        name: "example-template".into(),
        path: ".rigorix/templates/example-template.toml".into(),
        status: "created".into(),
    }
}

/// Example validation output.
pub fn example_validate_template_output() -> ValidateTemplateOutput {
    ValidateTemplateOutput {
        valid: true,
        warnings: vec![],
        errors: vec![],
        estimated_cost: None,
    }
}

/// List of all registered template tool names.
pub const TEMPLATE_TOOL_NAMES: &[&str] = &[
    "rigorix_list_templates",
    "rigorix_get_template",
    "rigorix_create_template",
    "rigorix_validate_template",
];
