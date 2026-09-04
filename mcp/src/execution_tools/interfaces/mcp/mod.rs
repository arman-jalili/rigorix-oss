//! MCP protocol handler contracts for Execution Tools.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#mcp-handlers
//! Implements: Contract Freeze — rigorix_execute, rigorix_validate_plan,
//! rigorix_check_enforcement tool handler contracts
//!
//! These contracts define the MCP tool schema registrations for the
//! execution tools. Each handler defines:
//! - Tool name (as registered in ToolRegistry)
//! - Input JSON schema
//! - Output format
//! - Error conditions
//!
//! # Contract (Frozen)
//!
//! - Tool names are frozen (rigorix_execute, rigorix_validate_plan, rigorix_check_enforcement)
//! - Input schemas are documented but not enforced by types here
//! - Output format follows MCP ToolResult specification
//! - Error format follows HandlerError type
//!
//! # API Endpoints
//!
//! | Method | Tool Name | Handler | Description |
//! |--------|-----------|---------|-------------|
//! | tools/call | `rigorix_execute` | ExecuteHandler | Execute a plan through rigorix-engine |
//! | tools/call | `rigorix_validate_plan` | ValidatePlanHandler | Validate a plan against policies |
//! | tools/call | `rigorix_check_enforcement` | CheckEnforcementHandler | Check enforcement status |

use serde_json::json;

use crate::execution_tools::application::dto::{
    CheckEnforcementOutput, ExecuteOutput, ValidateOutput,
};

// ---------------------------------------------------------------------------
// Tool Schema Definitions
// ---------------------------------------------------------------------------

/// JSON Schema for the `rigorix_execute` tool input.
pub const RIGORIX_EXECUTE_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "plan": {
            "type": "object",
            "description": "The plan to execute with steps, constraints, and metadata. Omit if template_name is provided.",
            "properties": {
                "name": { "type": "string", "description": "Plan name" },
                "description": { "type": "string", "description": "Plan description" },
                "steps": {
                    "type": "array",
                    "description": "Ordered list of steps to execute",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "tool": { "type": "string" },
                            "parameters": { "type": "object" },
                            "requires_approval": { "type": "boolean" },
                            "description": { "type": "string" },
                            "timeout_secs": { "type": "integer" },
                            "evaluate_score": { "type": "boolean", "description": "Whether to run scored evaluation on this step's output" }
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
        "template_name": {
            "type": "string",
            "description": "Name of an existing template to load and execute (created via rigorix_create_template). Use this instead of plan to execute a previously registered template."
        },
        "execution_id": {
            "type": "string",
            "format": "uuid",
            "description": "Optional pre-generated execution ID for idempotency"
        },
        "repository": {
            "type": "string",
            "description": "Repository name for audit (e.g. 'my-org/my-repo')"
        },
        "author": {
            "type": "string",
            "description": "Author identity for audit (e.g. email or username)"
        }
    }
}"#;

/// JSON Schema for the `rigorix_validate_plan` tool input.
pub const RIGORIX_VALIDATE_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "plan": {
            "type": "object",
            "description": "The plan to validate against enforcement policies",
            "properties": {
                "name": { "type": "string" },
                "description": { "type": "string" },
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
                            "timeout_secs": { "type": "integer" },
                            "evaluate_score": { "type": "boolean", "description": "Whether to run scored evaluation on this step's output" }
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

/// JSON Schema for the `rigorix_check_enforcement` tool input.
pub const RIGORIX_CHECK_ENFORCEMENT_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {},
    "description": "No input parameters required"
}"#;

/// JSON Schema for the `rigorix_plan` tool input.
pub const RIGORIX_PLAN_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "template_name": {
            "type": "string",
            "description": "Name of the template to plan. The template must exist in .rigorix/templates/. Returns the planned DAG with enforcement validation without executing."
        }
    },
    "required": ["template_name"]
}"#;

/// JSON Schema for the `rigorix_run` tool input.
pub const RIGORIX_RUN_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "template_name": {
            "type": "string",
            "description": "Name of the template to execute. The template must exist in .rigorix/templates/. Executes the template's DAG and returns results with audit trail."
        },
        "execution_id": {
            "type": "string",
            "format": "uuid",
            "description": "Optional pre-generated execution ID for idempotency"
        },
        "repository": {
            "type": "string",
            "description": "Repository name for audit (e.g. 'my-org/my-repo')"
        },
        "author": {
            "type": "string",
            "description": "Author identity for audit (e.g. email or username)"
        }
    },
    "required": ["template_name"]
}"#;

// ---------------------------------------------------------------------------
// MCP Tool Descriptors
// ---------------------------------------------------------------------------

/// Descriptor for the `rigorix_execute` tool.
///
/// Used for registering the tool schema in ToolRegistry.
pub fn rigorix_execute_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_execute",
        "description": "Execute a structured plan through the rigorix engine. Validates the plan against enforcement policies, executes each step in order, and returns execution results with audit trail.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_EXECUTE_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

/// Descriptor for the `rigorix_validate_plan` tool.
pub fn rigorix_validate_plan_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_validate_plan",
        "description": "Validate a plan against enforcement policies without executing it. Returns validation warnings, blocking errors, and estimated cost.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_VALIDATE_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

/// Descriptor for the `rigorix_check_enforcement` tool.
pub fn rigorix_check_enforcement_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_check_enforcement",
        "description": "Check current enforcement status including active preset, remaining budget (tool calls and tokens), and circuit breaker states.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_CHECK_ENFORCEMENT_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

/// JSON Schema for the `rigorix_approve_execution` tool input.
pub const RIGORIX_APPROVE_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "execution_id": {
            "type": "string",
            "format": "uuid",
            "description": "Execution ID of a run that returned status PendingApproval"
        },
        "step_names": {
            "type": "array",
            "items": { "type": "string" },
            "minItems": 1,
            "description": "Step names to approve (human sign-off). Steps that declared requires_approval: true only run after approval."
        },
        "approver_id": {
            "type": "string",
            "description": "Optional — identity subject of the human approving. Required when the ADR-011 approval binding is enabled (R3: identity is a captured fact; the engine denies approval without it)."
        },
        "authority": {
            "type": "string",
            "description": "Optional — role/policy id of the approver (captured fact)."
        },
        "token_claims_ref": {
            "type": "string",
            "description": "Optional — IdP token/claims presented at approval (credential-substitution check)."
        }
    },
    "required": ["execution_id", "step_names"]
}"#;

/// Descriptor for the `rigorix_approve_execution` tool.
///
/// Provides the human sign-off half of the `requires_approval` plan
/// contract: approves steps of a paused execution and resumes it.
pub fn rigorix_approve_execution_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_approve_execution",
        "description": "Approve steps of an execution paused for human sign-off (status PendingApproval) and resume it. Steps that declared requires_approval: true are only executed after approval. Returns approved, not-found, still-pending step names and whether the execution resumed.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_APPROVE_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

/// Descriptor for the `rigorix_plan` tool.
pub fn rigorix_plan_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_plan",
        "description": "Resolve a template from .rigorix/templates/ and display the planned DAG without execution. Validates the plan against enforcement policies and shows the step graph, constraints, and enforcement status. Use this before rigorix_run to preview what will execute.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_PLAN_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

/// Descriptor for the `rigorix_run` tool.
pub fn rigorix_run_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_run",
        "description": "Load a template from .rigorix/templates/ and execute its DAG through rigorix-engine. Returns execution results with per-step status, duration, and audit URI.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_RUN_INPUT_SCHEMA).expect("schema const is valid JSON (test-enforced)")
    })
}

// ---------------------------------------------------------------------------
// Example output formats
// ---------------------------------------------------------------------------

/// Example successful execution output.
pub fn example_execute_output() -> ExecuteOutput {
    ExecuteOutput {
        execution_id: uuid::Uuid::nil(),
        status: "completed".into(),
        steps: vec![],
        duration_ms: 0,
        tokens_used: None,
        audit_uri: "rigorix://audit/00000000-0000-0000-0000-000000000000".into(),
    }
}

/// Example validation output.
pub fn example_validate_output() -> ValidateOutput {
    ValidateOutput {
        valid: true,
        warnings: vec![],
        errors: vec![],
        estimated_cost: None,
    }
}

/// Example enforcement check output.
pub fn example_check_enforcement_output() -> CheckEnforcementOutput {
    CheckEnforcementOutput {
        active: true,
        preset: "default".into(),
        budget: crate::execution_tools::application::dto::BudgetDto {
            tool_calls_total: 1000,
            tool_calls_remaining: 750,
            tokens_total: 100000,
            tokens_remaining: 75000,
        },
        circuit_breakers: vec![],
    }
}

/// List of all registered execution tool names.
pub const EXECUTION_TOOL_NAMES: &[&str] = &[
    "rigorix_execute",
    "rigorix_plan",
    "rigorix_run",
    "rigorix_validate_plan",
    "rigorix_check_enforcement",
];
