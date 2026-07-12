//! MCP protocol handler contracts for the Enterprise Proxy.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#mcp-handlers
//! Implements: Contract Freeze — rigorix_enterprise_* tool handler contracts
//!
//! These contracts define the MCP tool schema registrations for the
//! enterprise proxy. Unlike the statically compiled template/execution tools,
//! enterprise tools are **dynamically discovered** from the enterprise API
//! during initialization. The contracts here define how tools are registered,
//! routed, and how errors are formatted.
//!
//! # Contract (Frozen)
//!
//! - Tool naming convention is `rigorix_enterprise_*`
//! - Input schemas are dynamically retrieved from enterprise API
//! - Error format follows MCP ToolResult specification with clear diagnostics
//! - All enterprise tool calls go through a single handler with method routing
//!
//! # API Endpoints
//!
//! | Method | Tool Name Pattern | Handler | Description |
//! |--------|-------------------|---------|-------------|
//! | tools/call | `rigorix_enterprise_*` | EnterpriseToolRouter | Proxy tool call to enterprise API |
//! | tools/list | (dynamic schemas) | EnterpriseToolRouter | Dynamically listed from schema cache |

use serde_json::json;

use crate::enterprise_proxy::application::dto::{
    HandleToolCallOutput, HealthCheckOutput, InitializeOutput, ListAvailableToolsOutput,
    ToolSchemaDto,
};

// ---------------------------------------------------------------------------
// Enterprise Tool Prefix
// ---------------------------------------------------------------------------

/// Prefix for all enterprise tool names.
pub const ENTERPRISE_TOOL_PREFIX: &str = "rigorix_enterprise_";

// ---------------------------------------------------------------------------
// Core Tool Descriptors
// ---------------------------------------------------------------------------

/// Descriptor for the `rigorix_enterprise_call` tool — the main proxy tool.
///
/// This is the only statically compiled enterprise tool. It accepts any
/// enterprise method and params, and proxies them to the enterprise API.
/// Dynamic tools discovered from the API are also registered separately.
pub fn rigorix_enterprise_call_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_enterprise_call",
        "description": "Call any enterprise API method. Dynamically discovered enterprise tools (e.g., rigorix_enterprise_team_audit, rigorix_enterprise_approve) are also registered for direct use. This is a universal proxy that accepts method + params.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "pattern": "^rigorix_enterprise_",
                    "description": "Enterprise tool method name (e.g., rigorix_enterprise_team_audit)"
                },
                "params": {
                    "type": "object",
                    "description": "Tool-specific parameters as a JSON object"
                }
            },
            "required": ["method", "params"]
        }
    })
}

/// Descriptor for the `rigorix_enterprise_health` tool.
pub fn rigorix_enterprise_health_tool_descriptor() -> serde_json::Value {
    json!({
        "name": "rigorix_enterprise_health",
        "description": "Check the health and connectivity status of the enterprise API. Returns latency, version, and status message.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "description": "No input parameters required"
        }
    })
}

// ---------------------------------------------------------------------------
// Error Response Formats
// ---------------------------------------------------------------------------

/// Format a standardized enterprise proxy error response.
///
/// All enterprise tool errors follow this format to ensure
/// clear, actionable diagnostics for the end user.
pub fn format_enterprise_error(error_type: &str, message: &str) -> serde_json::Value {
    json!({
        "error": {
            "type": error_type,
            "message": message,
            "enterprise_proxy": true,
            "resolution_hint": match error_type {
                "not_enabled" => "Configure enterprise.api_url and enterprise.api_key to enable enterprise features.",
                "auth_failure" => "Verify your enterprise API key is valid and not expired.",
                "timeout" => "The enterprise API is unreachable or too slow. Check network connectivity and API endpoint.",
                "api_error" => "The enterprise API returned an error. Check the API server logs.",
                "network_error" => "Cannot reach the enterprise API. Check network connectivity and firewall rules.",
                _ => "Contact your enterprise administrator for assistance.",
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Example Outputs
// ---------------------------------------------------------------------------

/// Example successful initialization output.
pub fn example_initialize_output() -> InitializeOutput {
    InitializeOutput {
        success: true,
        tool_count: 4,
        version: "1.0.0".into(),
        server_name: "Rigorix Enterprise Server".into(),
    }
}

/// Example successful health check output.
pub fn example_health_check_output() -> HealthCheckOutput {
    HealthCheckOutput {
        healthy: true,
        latency_ms: 42,
        version: "1.0.0".into(),
        message: "Enterprise API is healthy".into(),
    }
}

/// Example successful tool call output.
pub fn example_handle_tool_call_output() -> HandleToolCallOutput {
    HandleToolCallOutput {
        success: true,
        result: Some(json!({
            "status": "ok",
            "data": { "team_id": "123", "audit_count": 5 }
        })),
        error: None,
        duration_ms: 156,
    }
}

/// Example tool listing output.
pub fn example_list_tools_output() -> ListAvailableToolsOutput {
    ListAvailableToolsOutput {
        tools: vec![ToolSchemaDto {
            name: "rigorix_enterprise_team_audit".into(),
            description: "Audit team activity and compliance".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "team_id": { "type": "string" },
                    "since": { "type": "string", "format": "date-time" },
                    "until": { "type": "string", "format": "date-time" },
                    "limit": { "type": "integer" }
                },
                "required": ["team_id"]
            }),
        }],
        tool_count: 1,
    }
}

/// List of all statically registered enterprise tool names.
pub const ENTERPRISE_TOOL_NAMES: &[&str] =
    &["rigorix_enterprise_call", "rigorix_enterprise_health"];
