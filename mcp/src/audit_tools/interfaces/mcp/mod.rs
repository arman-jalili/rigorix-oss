//! MCP protocol handler contracts for Audit Tools.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#mcp-handlers
//! Implements: Contract Freeze — rigorix_read_audit, rigorix_list_audits,
//! rigorix_audit_summary tool handler contracts
//!
//! These contracts define the MCP tool schema registrations for the
//! audit tools. Each handler defines:
//! - Tool name (as registered in ToolRegistry)
//! - Input JSON schema
//! - Output format
//! - Error conditions
//!
//! # Contract (Frozen)
//!
//! - Tool names are frozen (rigorix_read_audit, rigorix_list_audits, rigorix_audit_summary)
//! - Input schemas are documented but not enforced by types here
//! - Output format follows MCP ToolResult specification
//! - Error format follows AuditHandlerError type
//!
//! # API Endpoints
//!
//! | Method | Tool Name | Handler | Description |
//! |--------|-----------|---------|-------------|
//! | tools/call | `rigorix_read_audit` | ReadAuditHandler | Read an audit record by execution ID |
//! | tools/call | `rigorix_list_audits` | ListAuditsHandler | List recent audit records with filtering |
//! | tools/call | `rigorix_audit_summary` | AuditSummaryHandler | Generate aggregate audit statistics |

use serde_json::json;

use crate::audit_tools::application::dto::{AuditSummaryOutput, ListAuditsOutput, ReadAuditOutput};

// ---------------------------------------------------------------------------
// Tool Schema Definitions
// ---------------------------------------------------------------------------

/// JSON Schema for the `rigorix_read_audit` tool input.
pub const RIGORIX_READ_AUDIT_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "execution_id": {
            "type": "string",
            "format": "uuid",
            "description": "The execution ID to query (UUID v4)"
        },
        "format": {
            "type": "string",
            "enum": ["text", "json"],
            "description": "Output format: 'text' (human-readable markdown, default) or 'json' (structured)"
        }
    },
    "required": ["execution_id"]
}"#;

/// JSON Schema for the `rigorix_list_audits` tool input.
pub const RIGORIX_LIST_AUDITS_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "status": {
            "type": "string",
            "enum": ["Completed", "Failed", "PartialFailed", "Cancelled", "EnforcementBlocked"],
            "description": "Filter by execution status"
        },
        "since": {
            "type": "string",
            "format": "date-time",
            "description": "Include records on or after this ISO 8601 timestamp"
        },
        "until": {
            "type": "string",
            "format": "date-time",
            "description": "Include records on or before this ISO 8601 timestamp"
        },
        "template": {
            "type": "string",
            "description": "Filter by template name (exact match)"
        },
        "limit": {
            "type": "integer",
            "minimum": 1,
            "maximum": 200,
            "default": 50,
            "description": "Maximum number of records to return (default: 50, max: 200)"
        }
    }
}"#;

/// JSON Schema for the `rigorix_audit_summary` tool input.
pub const RIGORIX_AUDIT_SUMMARY_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "since": {
            "type": "string",
            "format": "date-time",
            "description": "Start of the time range (ISO 8601). Defaults to 7 days ago."
        },
        "until": {
            "type": "string",
            "format": "date-time",
            "description": "End of the time range (ISO 8601). Defaults to current time."
        }
    }
}"#;

// ---------------------------------------------------------------------------
// MCP Tool Descriptors
// ---------------------------------------------------------------------------

/// Descriptor for the `rigorix_read_audit` tool.
///
/// Used for registering the tool schema in ToolRegistry.
pub fn rigorix_read_audit_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_read_audit",
        "description": "Read an execution audit record by execution ID. Returns full execution metadata, step results, token usage, and event history. Supports both human-readable text and structured JSON output.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_READ_AUDIT_INPUT_SCHEMA).unwrap()
    })
}

/// Descriptor for the `rigorix_list_audits` tool.
pub fn rigorix_list_audits_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_list_audits",
        "description": "List recent execution audit records with optional filtering by status, time range, and template name. Returns results ordered by completion time (newest first).",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_LIST_AUDITS_INPUT_SCHEMA).unwrap()
    })
}

/// Descriptor for the `rigorix_audit_summary` tool.
pub fn rigorix_audit_summary_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_audit_summary",
        "description": "Generate aggregate audit statistics over a time range. Returns total executions, success/failure counts, success rate, total duration, token usage, top failure patterns, and most frequently used templates.",
        "inputSchema": serde_json::from_str::<serde_json::Value>(RIGORIX_AUDIT_SUMMARY_INPUT_SCHEMA).unwrap()
    })
}

// ---------------------------------------------------------------------------
// Example output formats
// ---------------------------------------------------------------------------

/// Example successful read_audit output.
pub fn example_read_audit_output() -> ReadAuditOutput {
    ReadAuditOutput {
        execution_id: uuid::Uuid::nil(),
        status: "Completed".into(),
        template_name: Some("code-review".into()),
        started_at: "2026-07-12T10:00:00Z".into(),
        completed_at: "2026-07-12T10:05:30Z".into(),
        duration_ms: 330000,
        steps: vec![],
        tokens_used: Some(1500),
        audit_uri: "rigorix://audit/00000000-0000-0000-0000-000000000000".into(),
    }
}

/// Example list_audits output.
pub fn example_list_audits_output() -> ListAuditsOutput {
    ListAuditsOutput {
        total_count: 1,
        audits: vec![],
    }
}

/// Example audit summary output.
pub fn example_audit_summary_output() -> AuditSummaryOutput {
    AuditSummaryOutput {
        since: "2026-07-05T00:00:00Z".into(),
        until: "2026-07-12T00:00:00Z".into(),
        total_executions: 42,
        success_count: 38,
        failure_count: 4,
        success_rate: 0.9047,
        total_duration_ms: 12500000,
        total_tokens: Some(85000),
        top_failures: vec![],
        top_templates: vec![],
    }
}

/// List of all registered audit tool names.
pub const AUDIT_TOOL_NAMES: &[&str] = &[
    "rigorix_read_audit",
    "rigorix_list_audits",
    "rigorix_audit_summary",
];
