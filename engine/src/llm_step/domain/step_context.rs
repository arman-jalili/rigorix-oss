//! LlmStepContext — Provides the LLM with source code and failure context.
//!
//! @canonical .pi/architecture/modules/llm-step.md#llmstepcontext
//! Implements: Contract Freeze — LlmStepContext domain entity
//! Issue: issue-contract-freeze
//!
//! The LlmStepContext assembles the context that an LLM needs to generate
//! code or fix errors during DAG execution. It pulls data from the repo
//! engine (symbol graph, source files) and failure classification module
//! (previous failures, retry strategies).
//!
//! # Context Sources
//!
//! - **Source Code Context** — Relevant source files, symbol definitions,
//!   and code structure from the repo engine's code graph.
//! - **Failure Analysis** — Previous failure type, error messages, and
//!   retry strategy from the failure classification module.
//! - **Execution State** — Current DAG execution state, previous node
//!   outputs, and pending tasks from the execution engine.
//!
//! # Contract (Frozen)
//! - Pure domain entity with no framework dependencies
//! - Context is assembled from multiple sources; each source may be optional
//! - The assembled prompt is deterministic given the same inputs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Assembled context for an LLM generation step.
///
/// `LlmStepContext` is the result of gathering all relevant context
/// for an LLM generation. It contains the source code snippets, failure
/// analysis, and execution state that the LLM needs to generate a
/// meaningful response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStepContext {
    /// The generation node ID this context was built for.
    pub node_id: Uuid,

    /// The execution ID this node belongs to.
    pub execution_id: Uuid,

    /// Source code context gathered from the repo engine.
    pub source_context: SourceContext,

    /// Failure analysis context (empty if this is not a recovery step).
    pub failure_context: Option<FailureContext>,

    /// Execution state context.
    pub execution_context: ExecutionContext,

    /// ISO 8601 timestamp when this context was assembled.
    pub assembled_at: DateTime<Utc>,

    /// The assembled system prompt for the LLM.
    ///
    /// This is the final prompt that combines the prompt template with
    /// all context placeholders filled in.
    pub assembled_prompt: String,
}

/// Source code context gathered from the repo engine.
///
/// Contains the relevant source files, symbol definitions, and code
/// structure needed for LLM generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceContext {
    /// The relevant source files with their contents.
    ///
    /// Each entry maps a relative file path to its content snippet.
    pub files: Vec<SourceFileContext>,

    /// Symbol definitions relevant to the generation task.
    ///
    /// These are the symbols (functions, types, modules) that the LLM
    /// may need to reference or generate code for.
    pub symbols: Vec<SymbolDefinition>,

    /// The root directory of the repository being modified.
    pub repo_root: String,

    /// The target file path where generated code will be written (if known).
    pub target_file_path: Option<String>,
}

/// Context for a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFileContext {
    /// Relative path of the file within the repository.
    pub path: String,

    /// The content of the file (or a relevant snippet).
    pub content: String,

    /// The programming language of the file.
    pub language: String,

    /// Line range included (if this is a snippet).
    pub line_range: Option<(usize, usize)>,

    /// Whether this is the full file content or a snippet.
    pub is_full_file: bool,
}

/// A symbol definition from the code graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDefinition {
    /// The symbol name.
    pub name: String,

    /// The symbol kind (function, struct, enum, trait, module, etc.).
    pub kind: String,

    /// The file path where this symbol is defined.
    pub file_path: String,

    /// The signature / declaration of the symbol.
    pub signature: String,

    /// Documentation comment for the symbol (if available).
    pub doc_comment: Option<String>,
}

/// Failure analysis context for recovery generation.
///
/// Present when the LlmGenerateNode is being used for automatic
/// error recovery (fixing compilation errors, test failures, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureContext {
    /// The type of failure that triggered this generation.
    pub failure_type: String,

    /// The error message from the failure.
    pub error_message: String,

    /// The error output (e.g., compiler output, stack trace).
    pub error_output: String,

    /// The number of retries already attempted.
    pub retries_attempted: u8,

    /// The maximum number of retries configured.
    pub max_retries: u8,

    /// The retry strategy being applied.
    pub strategy: String,

    /// Previous generation outputs from prior retries (if any).
    pub previous_attempts: Vec<PreviousAttempt>,

    /// Additional context about the failure scenario.
    pub scenario_context: Option<String>,
}

/// A previous generation attempt for retry context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviousAttempt {
    /// The attempt number (1-indexed).
    pub attempt: u8,

    /// The output generated in this attempt.
    pub output: String,

    /// The error that resulted from this attempt.
    pub error: String,

    /// ISO 8601 timestamp of this attempt.
    pub attempted_at: DateTime<Utc>,
}

/// Execution state context.
///
/// Provides the LLM with information about the current DAG execution
/// state, including previous node outputs and pending tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// The ID of the containing DAG execution.
    pub dag_id: Uuid,

    /// The current phase of the DAG execution.
    pub execution_phase: String,

    /// Outputs from previous nodes in the DAG (keyed by node ID).
    pub previous_node_outputs: Vec<NodeOutput>,
}

/// Output from a previously executed node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeOutput {
    /// The node ID.
    pub node_id: Uuid,

    /// The node name.
    pub node_name: String,

    /// The output produced by this node.
    pub output: String,

    /// Whether the node completed successfully.
    pub success: bool,
}

impl LlmStepContext {
    /// Check if this context includes failure analysis data.
    pub fn has_failure_context(&self) -> bool {
        self.failure_context.is_some()
    }

    /// Get the number of source files included in the context.
    pub fn source_file_count(&self) -> usize {
        self.source_context.files.len()
    }

    /// Get the total character length of all source files combined.
    pub fn total_source_size(&self) -> usize {
        self.source_context
            .files
            .iter()
            .map(|f| f.content.len())
            .sum()
    }
}

impl Default for SourceContext {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            symbols: Vec::new(),
            repo_root: String::new(),
            target_file_path: None,
        }
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            dag_id: Uuid::nil(),
            execution_phase: String::from("unknown"),
            previous_node_outputs: Vec::new(),
        }
    }
}
