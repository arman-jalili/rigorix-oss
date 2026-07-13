//! MCP tool handler for `rigorix_get_usage_guide`.
//!
//! Returns structured usage context about valid action types, intent formats,
//! workflow patterns, and plan JSON structure. Self-documenting — Claude can
//! call this at runtime to understand how to use rigorix correctly.

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Tool Schema
// ---------------------------------------------------------------------------

/// JSON Schema for `rigorix_get_usage_guide` input.
pub const RIGORIX_GET_USAGE_GUIDE_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {},
    "description": "Returns a comprehensive usage guide for all rigorix MCP tools including valid action types, workflow patterns, and plan JSON structure."
}"#;

// ---------------------------------------------------------------------------
// Tool Descriptor
// ---------------------------------------------------------------------------

/// Descriptor for the `rigorix_get_usage_guide` tool.
pub fn rigorix_get_usage_guide_tool_descriptor() -> Value {
    json!({
        "name": "rigorix_get_usage_guide",
        "description": "Get usage guide for rigorix MCP tools. Returns valid action types, intent formats, workflow patterns (create template → validate → execute → audit), and example plan JSON structures. Call this first if you are unfamiliar with the rigorix tool system.",
        "inputSchema": serde_json::from_str::<Value>(RIGORIX_GET_USAGE_GUIDE_INPUT_SCHEMA).unwrap()
    })
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Handle a `rigorix_get_usage_guide` tool call.
///
/// Accepts optional `jsonrpc` params (ignored — no required input).
/// Returns a structured JSON response with usage context.
pub fn handle_get_usage_guide() -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::to_string_pretty(&build_guide()).unwrap()
            }
        ]
    })
}

// ---------------------------------------------------------------------------
// Guide content
// ---------------------------------------------------------------------------

fn build_guide() -> Value {
    json!({
        "version": "1.0.0",
        "workflow": {
            "title": "Recommended Workflow",
            "description": "For best results, follow this workflow when using rigorix tools:",
            "steps": [
                {
                    "step": 1,
                    "action": "rigorix_create_template",
                    "description": "Create a template with action nodes (file_read, run_command, etc.). This defines what will be executed."
                },
                {
                    "step": 2,
                    "action": "rigorix_validate_plan",
                    "description": "Validate a plan against enforcement policies before executing. Catches errors early."
                },
                {
                    "step": 3,
                    "action": "rigorix_execute",
                    "description": "Execute the plan through rigorix-engine. Returns execution results with audit trail."
                },
                {
                    "step": 4,
                    "action": "rigorix_read_audit",
                    "description": "Read the audit record for a completed execution. Provides full traceability."
                }
            ]
        },
        "action_types": [
            {
                "type": "file_read",
                "description": "Read a file from the working directory",
                "intent_format": "Read <path>",
                "intent_example": "Read src/main.rs",
                "parameters": {
                    "path": "string — Path to the file (e.g. 'src/main.rs')"
                }
            },
            {
                "type": "file_write",
                "description": "Write content to a file (creates or overwrites)",
                "intent_format": "Write <content> to <path>",
                "intent_example": "Write 'Hello World' to README.md",
                "parameters": {
                    "path": "string — Path to the file (e.g. 'README.md')",
                    "content": "string — Content to write"
                }
            },
            {
                "type": "file_edit",
                "description": "Edit an existing file with old/new text replacement",
                "intent_format": "Replace <old> with <new> in <path>",
                "intent_example": "Replace 'foo' with 'bar' in src/lib.rs",
                "parameters": {
                    "path": "string — Path to the file",
                    "old_string": "string — Exact text to replace",
                    "new_string": "string — Replacement text"
                }
            },
            {
                "type": "run_command",
                "description": "Run a shell command and capture output",
                "intent_format": "Run <command>",
                "intent_example": "Run cargo test",
                "parameters": {
                    "command": "string — Shell command to execute",
                    "working_dir": "string (optional) — Working directory",
                    "timeout_secs": "integer (optional) — Timeout in seconds"
                }
            },
            {
                "type": "llm_step",
                "description": "Run an LLM query with system prompt and user prompt",
                "intent_format": "Answer <question> using <context>",
                "intent_example": "Explain the architecture using src/main.rs",
                "parameters": {
                    "system_prompt": "string — System prompt for the LLM",
                    "user_prompt": "string — User query",
                    "model": "string (optional) — Model override",
                    "temperature": "number (optional) — Temperature (0.0 to 1.0)"
                }
            },
            {
                "type": "search_code",
                "description": "Search codebase for a pattern",
                "intent_format": "Search for <pattern> in <scope>",
                "intent_example": "Search for EngineFacade in src/",
                "parameters": {
                    "pattern": "string — Search pattern (regex)",
                    "path": "string (optional) — Directory scope",
                    "max_results": "integer (optional) — Max results (default: 50)"
                }
            },
            {
                "type": "http_request",
                "description": "Make an HTTP/HTTPS request",
                "intent_format": "GET <url>",
                "intent_example": "GET https://api.example.com/health",
                "parameters": {
                    "method": "string — HTTP method (GET, POST, PUT, DELETE, etc.)",
                    "url": "string — Request URL",
                    "headers": "object (optional) — Request headers",
                    "body": "string (optional) — Request body"
                }
            },
            {
                "type": "wait_for_approval",
                "description": "Pause execution and wait for human approval. An approval prompt with reasoning is shown to the user",
                "intent_format": "Request approval to <action> because <reason>",
                "intent_example": "Request approval to deploy to production because staging tests passed",
                "parameters": {
                    "reason": "string — Explanation for why approval is needed",
                    "timeout_secs": "integer (optional) — Approval timeout"
                }
            },
            {
                "type": "condition",
                "description": "Evaluate a condition and branch execution",
                "intent_format": "If <condition> then <action> else <fallback>",
                "intent_example": "If cargo test passes then deploy else report failure",
                "parameters": {
                    "condition": "string — Expression to evaluate (e.g. 'previous_step.status == \"success\"')",
                    "on_true": "string — Node ID to route to if condition is true",
                    "on_false": "string — Node ID to route to if condition is false"
                }
            }
        ],
        "plan_json_structure": {
            "description": "Example plan JSON for rigorix_execute and rigorix_create_template:",
            "example": {
                "name": "example-plan",
                "description": "Example plan demonstrating required fields",
                "version": "1.0.0",
                "tags": ["example"],
                "steps": [
                    {
                        "name": "read-config",
                        "tool": "file_read",
                        "parameters": {
                            "path": "config.json"
                        },
                        "requires_approval": false,
                        "description": "Read the configuration file"
                    }
                ],
                "metadata": {},
                "created_at": "2026-07-13T00:00:00Z",
                "updated_at": "2026-07-13T00:00:00Z"
            },
            "required_fields": [
                "name (string) — Unique plan name",
                "description (string) — Plan description",
                "version (string) — Semantic version",
                "tags (array of strings) — Categorization tags",
                "steps (array) — Ordered list of execution steps",
                "metadata (object) — Extensible key-value metadata",
                "created_at (ISO 8601) — Creation timestamp",
                "updated_at (ISO 8601) — Last update timestamp"
            ],
            "step_object": {
                "required_fields": [
                    "name (string) — Step name",
                    "tool (string) — One of the valid action types above",
                    "parameters (object) — Tool-specific parameters",
                    "requires_approval (boolean) — Whether human approval is needed",
                    "description (string) — Step description"
                ],
                "optional_fields": [
                    "timeout_secs (integer) — Step-specific timeout override"
                ]
            }
        },
        "template_file_format": {
            "description": "Templates can be created as .toml files on disk or via rigorix_create_template:",
            "example_toml_path": ".rigorix/templates/<template-name>.toml",
            "example_toml": r#"[template]
id = "example"
name = "Example Template"
description = "A sample template"
version = "1.0.0"
category = "General"

[[template.parameters]]
name = "file_path"
type = "string"
required = true
description = "Path to the file"

[[template.nodes]]
id = "step-1"
name = "Read File"
action = { type = "file_read", path = "{{ file_path }}" }
description = "Read the specified file"
"#,
            "disk_location": ".rigorix/templates/ (relative to working directory)"
        },
        "available_tools": {
            "description": "All registered MCP tools and their purposes:",
            "execution_tools": [
                "rigorix_execute — Execute a structured plan through rigorix-engine",
                "rigorix_validate_plan — Validate a plan against enforcement policies",
                "rigorix_check_enforcement — Check current enforcement status and budget"
            ],
            "template_tools": [
                "rigorix_list_templates — List all registered templates",
                "rigorix_get_template — Get a specific template by name",
                "rigorix_create_template — Create or update a template with a plan definition",
                "rigorix_validate_template — Validate a template TOML against schema"
            ],
            "audit_tools": [
                "rigorix_read_audit — Read an execution audit record",
                "rigorix_list_audits — List all execution audits",
                "rigorix_audit_summary — Get an audit summary with statistics"
            ],
            "enterprise_tools": [
                "rigorix_enterprise_call — Proxy a tool call to an enterprise endpoint (requires ENTERPRISE_API_URL)",
                "rigorix_enterprise_health — Check enterprise endpoint health (requires ENTERPRISE_API_URL)"
            ],
            "guide_tool": [
                "rigorix_get_usage_guide — This tool — get usage context for all rigorix tools"
            ]
        }
    })
}
