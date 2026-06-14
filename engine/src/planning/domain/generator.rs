//! TemplateGenerator trait — fallback template generation from user intent.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#generator
//! Implements: Contract Freeze — TemplateGenerator trait
//! Issue: issue-contract-freeze
//!
//! TemplateGenerator is the fallback path in the planning pipeline.
//! When the Classifier finds no good match (confidence < 0.3 for all
//! templates), the pipeline falls back to the TemplateGenerator to
//! create a new template definition on-the-fly from the user intent.
//!
//! # Contract (Frozen)
//! - Generates a TOML template string from user intent
//! - The generated template must be parseable by TemplateParserService
//! - The generated template is registered in the TemplateEngine before re-classifying
//! - Implementations must be deterministic (same intent → same template structure)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::intent::UserIntent;

/// Generates a new template definition from user intent.
///
/// Used as a fallback when no existing template matches the user's
/// intent. The generator creates a complete TOML template definition
/// that can be parsed and registered for immediate use.
///
/// # Contract (Frozen)
/// - `generate` returns a TOML string matching the template schema
/// - The output must be parseable by `TemplateParserService::parse_str`
/// - Budget is consumed via `LlmBudget` reservation
/// - Implementations should include appropriate node structure and parameters
#[async_trait]
pub trait TemplateGenerator: Send + Sync {
    /// Generate a template definition from user intent.
    ///
    /// Creates a TOML template string that the TemplateEngine can
    /// parse and register. The generated template should match the
    /// user's intent as closely as possible.
    ///
    /// # Arguments
    ///
    /// * `intent` — The user's raw intent (with optional clarifications).
    /// * `repo_context` — Repository snapshot with file tree, public API,
    ///   dependencies, and optional symbol graph for validation.
    /// * `budget` — LLM budget for tracking generation cost.
    ///
    /// # Returns
    ///
    /// A `GeneratedTemplate` containing the TOML string and metadata.
    async fn generate(
        &self,
        intent: &UserIntent,
        repo_context: &RepoContext,
        budget: &LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError>;

    /// Estimate the token cost of generating a template.
    ///
    /// Provides a rough estimate for budget pre-checking before
    /// the actual generation call.
    fn estimate_cost(&self, intent: &UserIntent) -> GeneratedTemplateCost;
}

/// A template generated on-the-fly by the TemplateGenerator.
///
/// Carries the TOML content, metadata, and any validation/symbol
/// validation results for the generated template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTemplate {
    /// The TOML string of the generated template definition.
    pub toml_content: String,

    /// Suggested template ID (used for registration).
    pub suggested_id: String,

    /// Suggested human-readable name.
    pub suggested_name: String,

    /// Brief description of what this template does.
    pub description: String,

    /// Number of LLM calls used.
    pub llm_calls_used: u32,

    /// Number of LLM tokens consumed.
    pub llm_tokens_used: u32,
}

/// Estimated cost of generating a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTemplateCost {
    /// Estimated number of LLM calls.
    pub estimated_calls: u32,

    /// Estimated number of LLM tokens.
    pub estimated_tokens: u32,
}

// ---------------------------------------------------------------------------
// RepoContext — Repository snapshot for generation context
// ---------------------------------------------------------------------------

/// Snapshot of repository structure used as context for template generation.
///
/// Provides the LLM with knowledge of the codebase structure,
/// public API surface, and existing dependencies to prevent
/// hallucinated types, fields, or method references.
///
/// # Contract (Frozen)
/// - `directory_tree` is a flat or nested listing of relevant files
/// - `public_api` lists public types, functions, and traits
/// - `dependencies` lists external crate/package references
/// - `symbol_graph_snapshot` is an optional subset of the indexed symbol graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoContext {
    /// Working directory being operated on.
    pub root_dir: PathBuf,

    /// Detected project type (e.g. "rust", "python", "typescript").
    pub project_type: String,

    /// Flat list of relevant file paths (relative to root_dir).
    pub directory_tree: Vec<String>,

    /// External dependencies (crate names, packages, etc.).
    pub dependencies: Vec<String>,

    /// Public type, function, and trait names available in the codebase.
    pub public_api: Vec<String>,

    /// Optional symbol graph subset for Phase 3 validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_graph_snapshot: Option<serde_json::Value>,
}

impl RepoContext {
    /// Create a new empty RepoContext for a given directory.
    pub fn new(root_dir: PathBuf, project_type: String) -> Self {
        Self {
            root_dir,
            project_type,
            directory_tree: Vec::new(),
            dependencies: Vec::new(),
            public_api: Vec::new(),
            symbol_graph_snapshot: None,
        }
    }

    /// Check if this context has any file entries.
    pub fn has_files(&self) -> bool {
        !self.directory_tree.is_empty()
    }

    /// Check if this context has any public API entries.
    pub fn has_public_api(&self) -> bool {
        !self.public_api.is_empty()
    }
}

// ---------------------------------------------------------------------------
// GeneratorError — Typed error enum for generation failures
// ---------------------------------------------------------------------------

/// Errors specific to the template generation process.
///
/// Separate from `PlanningError` because generation has distinct
/// failure modes (TOML parse, symbol validation, budget, LLM API)
/// that don't apply to the broader planning pipeline.
///
/// # Contract (Frozen)
/// - Every failure mode has a dedicated variant with structured context
/// - Errors carry enough information for meaningful retry feedback to the LLM
/// - Implements `std::error::Error` for library compatibility
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum GeneratorError {
    /// The LLM returned content that is not valid TOML.
    #[serde(rename = "invalid_toml")]
    InvalidToml {
        /// The raw LLM response that failed to parse.
        raw_response: String,
        /// The TOML parser error message.
        parse_error: String,
        /// Retry attempt number (0-based).
        attempt: u8,
    },

    /// The generated template failed structural validation.
    #[serde(rename = "validation_failed")]
    ValidationFailed {
        /// Template ID that failed validation.
        template_id: String,
        /// Validation error messages.
        errors: Vec<String>,
        /// Retry attempt number.
        attempt: u8,
    },

    /// Phase 3: Generated template references symbols that don't exist.
    #[serde(rename = "symbol_validation")]
    SymbolValidation {
        /// Template ID being validated.
        template_id: String,
        /// List of invalid symbol references found.
        invalid_references: Vec<InvalidSymbolReference>,
        /// Retry attempt number.
        attempt: u8,
    },

    /// The LLM budget was exhausted before generation completed.
    #[serde(rename = "budget_exhausted")]
    BudgetExhausted {
        /// Number of LLM calls consumed.
        calls_used: u32,
        /// Maximum allowed calls.
        max_calls: u32,
    },

    /// The LLM API call failed (network, auth, rate limit).
    #[serde(rename = "api_error")]
    ApiError {
        /// Human-readable error detail.
        detail: String,
        /// HTTP status code (if applicable).
        status_code: Option<u16>,
        /// Retry-after seconds (if rate limited).
        retry_after: Option<u64>,
    },

    /// Maximum retry attempts exhausted without generating a valid template.
    #[serde(rename = "max_retries_exhausted")]
    MaxRetriesExhausted {
        /// Number of attempts made.
        attempts: u8,
        /// Errors from each attempt.
        errors: Vec<String>,
    },

    /// The repository context could not be built.
    #[serde(rename = "context_build_failed")]
    ContextBuildFailed {
        /// Details about the failure.
        detail: String,
    },
}

/// An invalid symbol reference found during Phase 3 validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvalidSymbolReference {
    /// The symbol name that was referenced (e.g. "MyStruct", "some_field").
    pub symbol: String,

    /// How the symbol was used in the template (e.g. "type", "field_access").
    pub usage: String,

    /// The specific reason this reference is invalid.
    pub reason: String,

    /// Whether this reference uses `any` type (LLM escape hatch).
    pub is_any_type: bool,
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeneratorError::InvalidToml {
                raw_response,
                parse_error,
                attempt,
            } => write!(
                f,
                "Invalid TOML (attempt {}): {} - response: {}...",
                attempt,
                parse_error,
                &raw_response[..raw_response.len().min(100)]
            ),
            GeneratorError::ValidationFailed {
                template_id,
                errors,
                attempt,
            } => write!(
                f,
                "Validation failed for '{}' (attempt {}): {}",
                template_id,
                attempt,
                errors.join("; ")
            ),
            GeneratorError::SymbolValidation {
                template_id,
                invalid_references,
                attempt,
            } => write!(
                f,
                "Symbol validation failed for '{}' (attempt {}): {} invalid references",
                template_id,
                attempt,
                invalid_references.len()
            ),
            GeneratorError::BudgetExhausted {
                calls_used,
                max_calls,
            } => write!(
                f,
                "Budget exhausted: used {}/{} calls",
                calls_used, max_calls
            ),
            GeneratorError::ApiError {
                detail,
                status_code,
                retry_after,
            } => write!(
                f,
                "API error (status: {:?}, retry_after: {:?}): {}",
                status_code, retry_after, detail
            ),
            GeneratorError::MaxRetriesExhausted { attempts, errors } => {
                write!(
                    f,
                    "Max retries exhausted after {} attempts: {}",
                    attempts,
                    errors.join("; ")
                )
            }
            GeneratorError::ContextBuildFailed { detail } => {
                write!(f, "Context build failed: {}", detail)
            }
        }
    }
}

impl From<GeneratorError> for PlanningError {
    fn from(err: GeneratorError) -> Self {
        match err {
            GeneratorError::BudgetExhausted {
                calls_used,
                max_calls,
            } => PlanningError::BudgetExhausted {
                used_calls: calls_used,
                max_calls,
                used_tokens: 0,
                max_tokens: 0,
            },
            _ => PlanningError::TemplateEngineError {
                detail: err.to_string(),
            },
        }
    }
}
