//! MCP tool handler for `rigorix_get_usage_guide`.
//!
//! Returns structured usage context about valid action types, intent formats,
//! workflow patterns, and plan JSON structure. Self-documenting — Claude can
//! call this at runtime to understand how to use rigorix correctly.

use serde_json::{Value, json};

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
                    "action": "claude_writes_template",
                    "description": "Claude writes a .rigorix/templates/<name>.toml file directly using the Write tool. This defines the steps and their dependencies."
                },
                {
                    "step": 2,
                    "action": "rigorix_plan",
                    "description": "Load the template and display the planned DAG without executing. Shows graph nodes, dependencies, enforcement status, and estimated cost."
                },
                {
                    "step": 3,
                    "action": "rigorix_run",
                    "description": "Execute the template's DAG through rigorix-engine. Returns per-step results with audit trail for compliance."
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
                "type": "file_append",
                "description": "Append content to a file (creates if it doesn't exist)",
                "intent_format": "Append <content> to <path>",
                "intent_example": "Append 'new line' to src/main.rs",
                "parameters": {
                    "path": "string — Path to the file",
                    "content": "string — Content to append"
                }
            },
            {
                "type": "edit_file",
                "description": "Edit an existing file with old/new text replacement",
                "intent_format": "Replace <old> with <new> in <path>",
                "intent_example": "Replace 'foo' with 'bar' in src/lib.rs",
                "parameters": {
                    "path": "string — Path to the file",
                    "old_string": "string — Exact text to replace",
                    "new_string": "string — Replacement text",
                    "replace_all": "boolean (optional) — Replace all occurrences (default: false)"
                }
            },
            {
                "type": "file_patch",
                "description": "[PREFERRED for code injection] Insert or patch content using tree-sitter AST anchors or text search. Use this instead of run_command with sed/cat/regex scripts — tree-sitter anchors reliably find the right insertion point even when file content changes.",
                "intent_format": "Patch <path> with <insert> at <search>",
                "intent_example": "Patch src/task.ts inserting a new method before the TaskList class closing brace using anchor_type='class', anchor_name='TaskList', position='before'",
                "parameters": {
                    "path": "string — Path to the file",
                    "insert": "string — Content to insert",
                    "search": "string (optional) — Search text to locate insertion point",
                    "before": "boolean (optional) — Insert before search match (default: after)",
                    "anchor_type": "string (optional) — Tree-sitter node type for anchor-based insertion",
                    "anchor_name": "string (optional) — Tree-sitter node name for anchor-based insertion",
                    "container": "string (optional) — Container name to narrow anchor search",
                    "position": "string (optional) — 'before' or 'after' anchor (default: 'after')"
                }
            },
            {
                "type": "run_command",
                "description": "Run a shell command and capture output. Do NOT use for file patching/code injection — use file_patch with tree-sitter anchors instead.",
                "intent_format": "Run <command>",
                "intent_example": "Run cargo test",
                "parameters": {
                    "command": "string — Shell command to execute"
                }
            },
            {
                "type": "git_read",
                "description": "Run a read-only git command and capture output",
                "intent_format": "Run git <args>",
                "intent_example": "Run git diff --stat",
                "parameters": {
                    "args": "string — Git command arguments (e.g. 'diff', 'log --oneline', 'status')"
                }
            },
            {
                "type": "git_stage",
                "description": "Stage a file with git add",
                "intent_format": "Stage <path>",
                "intent_example": "Stage src/main.rs",
                "parameters": {
                    "path": "string — Path to the file to stage"
                }
            },
            {
                "type": "git_commit",
                "description": "Create a git commit with staged changes",
                "intent_format": "Commit <message>",
                "intent_example": "Commit 'Add new feature with tests'",
                "parameters": {
                    "message": "string — Commit message",
                    "auto_stage": "boolean (optional) — Stage all changes before commit (default: false)"
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
                "rigorix_plan — Load a template and display the planned DAG without execution",
                "rigorix_run — Load a template and execute its DAG through rigorix-engine",
                "rigorix_execute — Execute a plan or template through rigorix-engine (legacy, prefer plan+run)",
                "rigorix_validate_plan — Validate a plan against enforcement policies",
                "rigorix_check_enforcement — Check current enforcement status and budget",
                "rigorix_approve_execution — Approve steps of an execution paused for human sign-off (requires_approval) and resume it",
                "rigorix_evaluate_artifact — Rigorix scoring protocol MCP call. Sends artifact + rubric to a scoring backend. Returns multidimensional ScoringResult. Protocol is defined by Rigorix; external systems like RuntimeAI adopt it."
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
            ],
            "scored_evaluation": {
                "description": "Scored evaluation adds quality scoring of generated artifacts to the Rigorix DAG:",
                "data_flow": "ScoredEvaluationService -> Orchestrator -> BuildEnvelopeInput -> AuditEnvelope -> POST /api/v1/audit/oss-envelope -> audit_records JSONB -> mv_team_scoring -> GET /api/v1/reports/scoring -> Enterprise Dashboard",
                "protocol": "Rigorix defines the scoring protocol (rigorix_evaluate_artifact MCP request, rigorix_ping health check). External systems like RuntimeAI adopt this protocol by implementing the server side.",
                "example_template": "[[template.nodes]]\nid = \"score_output\"\nname = \"Score Code Quality\"\naction = { type = \"scored_evaluation\", backend = \"runtimeai\", rubric_source = \"inline\", rubric = { correctness = { threshold = 0.8 }, completeness = { threshold = 0.8 } } }\ndescription = \"Evaluate generated code quality\"\ndepends_on = [\"generate_code\"]",
                "scoring_result_schema": {
                    "passed": "boolean - Overall pass/fail",
                    "dimensions": "map<string, object> - Per-dimension scores",
                    "dimension.score": "number (0.0-1.0) - Achieved score",
                    "dimension.max": "number - Maximum possible score",
                    "dimension.label": "string - Human-readable label",
                    "dimension.passed": "boolean - Whether this dimension passed",
                    "summary": "string - Human-readable summary",
                    "backend": "string - Backend that produced the result",
                    "duration_ms": "number - Evaluation latency"
                },
                "enterprise_dashboard": "Scoring results appear under Reports > Scoring. GET /api/v1/reports/scoring returns: total_evaluations, scored_evaluations, pass_rate, avg_evaluation_duration_ms, dimension_breakdown (per-dimension avg_score and pass_rate).",
                "policy_conditions": {
                    "score_above": "ScoreAbove { dimension: Option<String>, threshold: u8 } - All dimensions above percentage threshold",
                    "score_below": "ScoreBelow { dimension: Option<String>, threshold: u8 } - Any dimension below percentage threshold",
                    "example_toml": "[[rules]]\nname = \"scored-evaluation-gate\"\ncondition = { type = \"score_below\", dimension = null, threshold = 80 }\naction = \"block_merge\""
                }
            }
        }
    })
}
