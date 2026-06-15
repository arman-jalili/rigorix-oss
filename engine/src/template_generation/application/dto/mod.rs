//! DTOs for the Template Generation module.
//!
//! @canonical .pi/architecture/modules/template-generation.md#dtos
//! Issue: issue-contract-freeze

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::template_generation::domain::{GeneratedTemplate, RepoContext};

/// Input for generating a new template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateTemplateInput {
    /// The execution session ID.
    pub session_id: Uuid,
    /// The user's intent (raw text).
    pub intent: String,
    /// Repository context for generation.
    pub repo_context: RepoContext,
    /// Whether to enable Phase 3 symbol validation.
    pub enable_symbol_validation: bool,
}

/// Output from generating a new template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateTemplateOutput {
    /// The generated template.
    pub generated: GeneratedTemplate,
    /// Whether symbol validation passed.
    pub symbol_validation_passed: bool,
    /// Symbol validation errors (if any).
    pub symbol_validation_errors: Vec<String>,
}

/// Input for symbol validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolValidationInput {
    /// The template TOML content to validate.
    pub template_content: String,
    /// The repo context with symbol graph snapshot.
    pub repo_context: RepoContext,
}

/// Output from symbol validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolValidationOutput {
    pub passed: bool,
    pub invalid_references: Vec<InvalidSymbolRef>,
}

/// An invalid symbol reference found during Phase 3 validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidSymbolRef {
    pub symbol: String,
    pub usage: String,
    pub reason: String,
}

/// Result of a symbol validation check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolValidationResult {
    pub passed: bool,
    pub invalid_references: Vec<InvalidSymbolRef>,
}

/// Estimated cost for generating a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateCostInput {
    pub intent: String,
}

/// Output with estimated costs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateCostOutput {
    pub estimated_calls: u32,
    pub estimated_tokens: u32,
}
