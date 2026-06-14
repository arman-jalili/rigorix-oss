//! Template domain entity, TemplateNode, ParameterDef, TemplateAction, and sub-types.
//!
//! @canonical .pi/architecture/modules/template-system.md#parser
//! Implements: Contract Freeze — Template aggregate with nodes, parameters, and action types
//! Issue: #101
//!
//! Defines the core domain types that represent a workflow template. Templates are
//! deserialized from TOML files, validated, and registered in the TemplateEngine for
//! runtime instantiation into executable DAGs.
//!
//! # TOML Schema (from ADR-002)
//!
//! ```toml
//! id = "unique-kebab-case"
//! name = "Human Name"
//! description = "What it does"
//! version = "1.0.0"
//!
//! [[parameters]]
//! name = "target_file"
//! description = "File to modify"
//! required = true
//! param_type = "path"
//!
//! [[nodes]]
//! id = "read-file"
//! name = "Read file"
//! depends_on = []
//! [nodes.action]
//! type = "file_read"
//! path = "{{ target_file }}"
//! ```
//!
//! # Contract (Frozen)
//! - Template is the root aggregate holding nodes and parameters
//! - All fields are public for direct access by application services
//! - Construction happens via TemplateParser (from TOML string/file)
//! - TemplateAction variants define all supported tool operations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Template (root aggregate)
// ---------------------------------------------------------------------------

/// A workflow template deserialized from a TOML definition file.
///
/// The root aggregate that holds metadata, parameter definitions, and
/// node definitions. Registered in the TemplateEngine for runtime
/// instantiation into executable TaskGraphs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    /// Unique kebab-case identifier (e.g. "read-file", "git-commit").
    pub id: String,

    /// Human-readable name (e.g. "Read File", "Git Commit").
    pub name: String,

    /// Description of what this template does.
    pub description: String,

    /// Semantic version of the template definition.
    pub version: String,

    /// Ordered list of parameter definitions for this template.
    #[serde(default)]
    pub parameters: Vec<ParameterDef>,

    /// Ordered list of node definitions that form the DAG structure.
    #[serde(default)]
    pub nodes: Vec<TemplateNode>,

    /// Optional tags for categorization and classification.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Optional category for grouping related templates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Optional author metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

impl Template {
    /// Look up a parameter by name.
    pub fn get_parameter(&self, name: &str) -> Option<&ParameterDef> {
        self.parameters.iter().find(|p| p.name == name)
    }

    /// Look up a node by ID.
    pub fn get_node(&self, node_id: &str) -> Option<&TemplateNode> {
        self.nodes.iter().find(|n| n.id == node_id)
    }

    /// Check if this template has any required parameters.
    pub fn has_required_params(&self) -> bool {
        self.parameters.iter().any(|p| p.required)
    }

    /// Get the set of parameter names used in placeholder substitutions
    /// across all nodes (e.g. extracts "{{ target_file }}" → ["target_file"]).
    pub fn referenced_params(&self) -> Vec<String> {
        let mut params: Vec<String> = Vec::new();
        for node in &self.nodes {
            params.extend(node.referenced_params());
        }
        params.sort();
        params.dedup();
        params
    }
}

// ---------------------------------------------------------------------------
// TemplateNode
// ---------------------------------------------------------------------------

/// A single node in a template DAG definition.
///
/// Each node represents one executable step with its action, dependencies,
/// retry policy, and optional validation rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateNode {
    /// Unique ID within this template (used for dependency references).
    pub id: String,

    /// Human-readable name for display and logging.
    pub name: String,

    /// IDs of nodes that must complete before this one.
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// The action this node performs.
    pub action: TemplateAction,

    /// Optional description of what this node does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Retry configuration for this node (inherits from template if not set).
    #[serde(default)]
    pub retry: RetryConfig,

    /// Optional post-execution validation rules.
    #[serde(default)]
    pub validate: Vec<ValidationRule>,

    /// Optional documentation intended for the LLM planning context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
}

impl TemplateNode {
    /// Extract all parameter references ({{ ... }}) from this node's action.
    pub fn referenced_params(&self) -> Vec<String> {
        self.action.referenced_params()
    }

    /// Check if this node has any dependencies.
    pub fn has_dependencies(&self) -> bool {
        !self.depends_on.is_empty()
    }
}

// ---------------------------------------------------------------------------
// TemplateAction
// ---------------------------------------------------------------------------

/// The action a template node performs.
///
/// Each variant corresponds to a supported tool operation. Actions carry
/// tool-specific fields as inline structured data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplateAction {
    /// Read a file from the working directory.
    FileRead {
        /// Path to the file (may contain {{ param }} placeholders).
        path: String,
    },

    /// Write content to a file (creates or overwrites).
    FileWrite {
        /// Path to the file (may contain {{ param }} placeholders).
        path: String,
        /// Content to write (may contain {{ param }} placeholders).
        content: String,
    },

    /// Append content to an existing file.
    FileAppend {
        /// Path to the file.
        path: String,
        /// Content to append.
        content: String,
    },

    /// Apply an AST-aware patch to a file.
    FilePatch {
        /// Path to the file.
        path: String,
        /// Search string or pattern to locate the insertion point.
        search: String,
        /// Content to insert.
        insert: String,
        /// Whether to insert before the search match (default: after).
        #[serde(default)]
        before: bool,
    },

    /// Execute a shell command with allowlist enforcement.
    RunCommand {
        /// The command to execute.
        command: String,
        /// Working directory (default: project root).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        /// Timeout in seconds.
        #[serde(default = "default_run_timeout")]
        timeout_secs: u64,
        /// Environment variable overrides.
        #[serde(default)]
        env: HashMap<String, String>,
    },

    /// Query the LSP for code intelligence (definitions, references, etc.).
    LspQuery {
        /// LSP query type (e.g. "goto-definition", "find-references").
        query_type: String,
        /// File path to query.
        file: String,
        /// Line number (0-indexed).
        line: u32,
        /// Column number (0-indexed).
        column: u32,
    },

    /// Read Git repository information.
    GitRead {
        /// Git command (e.g. "log", "diff --cached", "status").
        command: String,
        /// Path constraint (optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        /// Max results to return.
        #[serde(default = "default_git_max_results")]
        max_results: u32,
    },

    /// Stage files in Git.
    GitStage {
        /// Path(s) to stage (default: ".").
        #[serde(default = "default_git_stage_path")]
        path: String,
    },

    /// Create a Git commit.
    GitCommit {
        /// Commit message.
        message: String,
        /// Whether to automatically stage tracked files.
        #[serde(default)]
        auto_stage: bool,
    },
}

fn default_run_timeout() -> u64 {
    60
}

fn default_git_max_results() -> u32 {
    50
}

fn default_git_stage_path() -> String {
    ".".to_string()
}

impl TemplateAction {
    /// Extract all `{{ param_name }}` references from this action's fields.
    ///
    /// Looks for `{{ ... }}` patterns in the action's string fields.
    /// This is a simple character-based parser, no regex dependency needed.
    pub fn referenced_params(&self) -> Vec<String> {
        let mut params = Vec::new();
        let full_text = match self {
            TemplateAction::FileRead { path } => path.clone(),
            TemplateAction::FileWrite { path, content } => format!("{} {}", path, content),
            TemplateAction::FileAppend { path, content } => format!("{} {}", path, content),
            TemplateAction::FilePatch {
                path,
                search,
                insert,
                ..
            } => format!("{} {} {}", path, search, insert),
            TemplateAction::RunCommand { command, .. } => command.clone(),
            TemplateAction::LspQuery { file, .. } => file.clone(),
            TemplateAction::GitRead { command, .. } => command.clone(),
            TemplateAction::GitStage { path } => path.clone(),
            TemplateAction::GitCommit { message, .. } => message.clone(),
        };

        // Simple state-machine parser for {{ name }} patterns
        let bytes = full_text.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        while i + 1 < len {
            if bytes[i] == b'{' && bytes[i + 1] == b'{' {
                // Find closing }}
                let start = i + 2;
                let mut j = start;
                while j + 1 < len {
                    if bytes[j] == b'}' && bytes[j + 1] == b'}' {
                        let name = full_text[start..j].trim().to_string();
                        if !name.is_empty() && !params.contains(&name) {
                            params.push(name);
                        }
                        i = j + 2;
                        break;
                    }
                    j += 1;
                }
                // If no closing bracket found, advance
                if j == start && i < len {
                    i += 1;
                } else if j + 1 >= len {
                    i = len;
                }
            } else {
                i += 1;
            }
        }
        params
    }
}

// ---------------------------------------------------------------------------
// ParameterDef
// ---------------------------------------------------------------------------

/// Definition of a template parameter for user/LLM input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterDef {
    /// Parameter name used in `{{ name }}` placeholders.
    pub name: String,

    /// Human-readable description of the parameter.
    pub description: String,

    /// Whether this parameter is required (must be provided).
    #[serde(default)]
    pub required: bool,

    /// Expected type of the parameter value.
    #[serde(rename = "param_type", default)]
    pub param_type: ParamType,

    /// Default value if not provided (only for optional parameters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,

    /// Optional validation constraints.
    #[serde(default)]
    pub constraints: Vec<ParamConstraint>,
}

// ---------------------------------------------------------------------------
// ParamType
// ---------------------------------------------------------------------------

/// Expected type of a parameter value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParamType {
    /// A file system path (relative or absolute).
    #[serde(rename = "path")]
    Path,
    /// A string value.
    #[serde(rename = "string")]
    String,
    /// An integer number.
    #[serde(rename = "int")]
    Int,
    /// A floating-point number.
    #[serde(rename = "float")]
    Float,
    /// A boolean flag.
    #[serde(rename = "bool")]
    Bool,
    /// A selection from a predefined list of values.
    #[serde(rename = "enum")]
    Enum,
    /// Arbitrary JSON value.
    #[serde(rename = "json")]
    Json,
}

impl Default for ParamType {
    fn default() -> Self {
        ParamType::String
    }
}

// ---------------------------------------------------------------------------
// ParamConstraint
// ---------------------------------------------------------------------------

/// Validation constraint for a parameter value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParamConstraint {
    /// Minimum length (for strings) or minimum value (for numbers).
    Min { value: f64 },
    /// Maximum length (for strings) or maximum value (for numbers).
    Max { value: f64 },
    /// Regex pattern that the value must match (for strings).
    Pattern { regex: String },
    /// Allowed values (for enum types).
    AllowedValues { values: Vec<serde_json::Value> },
}

// ---------------------------------------------------------------------------
// RetryConfig
// ---------------------------------------------------------------------------

/// Retry configuration for a template node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries (0 = no retry).
    #[serde(default = "default_max_retries")]
    pub max_retries: u8,

    /// Which failure types should trigger a retry.
    #[serde(default)]
    pub retry_on: Vec<TemplateFailureType>,

    /// Retry strategy (same operation, expand context, etc.).
    #[serde(default)]
    pub strategy: RetryStrategy,

    /// Backoff delay in milliseconds between retries.
    #[serde(default = "default_backoff_ms")]
    pub backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            retry_on: vec![TemplateFailureType::Transient],
            strategy: RetryStrategy::SameOperation,
            backoff_ms: default_backoff_ms(),
        }
    }
}

fn default_max_retries() -> u8 {
    3
}

fn default_backoff_ms() -> u64 {
    100
}

/// Classification of failure types for retry decisions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TemplateFailureType {
    /// Transient failure (network timeout, rate limit).
    #[serde(rename = "transient")]
    Transient,
    /// LSP conflict (file changed while querying).
    #[serde(rename = "lsp_conflict")]
    LspConflict,
    /// Compilation error (after write/modification).
    #[serde(rename = "compile_error")]
    CompileError,
    /// Test failure.
    #[serde(rename = "test_failure")]
    TestFailure,
    /// Non-transient/unrecoverable error.
    #[serde(rename = "fatal")]
    Fatal,
}

/// Strategy for retrying a failed node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetryStrategy {
    /// Retry the same operation unchanged.
    #[serde(rename = "same_operation")]
    SameOperation,
    /// Expand search context and retry.
    #[serde(rename = "expand_context")]
    ExpandContext,
    /// Apply a patch/fix and retry.
    #[serde(rename = "patch_and_retry")]
    PatchAndRetry,
    /// Use a fallback node instead.
    #[serde(rename = "fallback")]
    Fallback,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        RetryStrategy::SameOperation
    }
}

// ---------------------------------------------------------------------------
// ValidationRule
// ---------------------------------------------------------------------------

/// Post-execution validation rule for a template node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidationRule {
    /// Run a lint pass on the affected files.
    #[serde(rename = "lint_pass")]
    LintPass {
        /// Files to lint (defaults to affected files).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        files: Option<Vec<String>>,
    },
    /// Run tests in the affected area.
    TestPass {
        /// Test filter (e.g. module name or test name pattern).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<String>,
    },
    /// Perform a type check (compile check).
    TypeCheck,
    /// Custom validation script/command.
    Custom {
        /// Command to run for validation.
        command: String,
        /// Expected exit code (default: 0).
        #[serde(default)]
        expected_exit_code: i32,
    },
}

// ---------------------------------------------------------------------------
// BuiltinTemplateMetadata
// ---------------------------------------------------------------------------

/// Metadata describing a built-in template definition.
///
/// Used by BuiltinTemplates to register the 13 built-in templates
/// that ship with Rigorix.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltinTemplateDescriptor {
    /// The template identifier.
    pub id: &'static str,

    /// Human-readable name.
    pub name: &'static str,

    /// Brief description.
    pub description: &'static str,

    /// Category grouping.
    pub category: TemplateCategory,

    /// TOML source content for this built-in template.
    pub toml_source: &'static str,
}

/// Category for grouping templates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TemplateCategory {
    /// File manipulation templates (read, write, patch, append).
    FileOperations,
    /// Git operations (stage, commit, log, diff).
    GitOperations,
    /// Code query templates (LSP, search).
    CodeQuery,
    /// Build and test templates.
    BuildTest,
    /// Custom/generic templates.
    Custom,
}
