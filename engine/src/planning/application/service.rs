//! Service interface (use case) for the Planning Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#pipeline
//! Implements: Contract Freeze — PlanningPipelineService trait
//! Issue: issue-contract-freeze
//!
//! The PlanningPipelineService trait defines the application-level operations
//! for orchestrating the 6-phase planning flow:
//!
//! 1. Budget Pre-check
//! 2. Intent Classification
//! 3. Parameter Extraction
//! 4. Graph Generation
//! 5. Plan Validation
//! 6. Hash Computation
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;
use uuid::Uuid;

use crate::planning::domain::classification::ClassificationResult;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::intent::UserIntent;
use crate::template_generation::domain::RepoContext;

use super::dto::{
    AvailableTemplatesOutput, BuildRepoContextInput, BuildRepoContextOutput, CheckBudgetInput,
    CheckBudgetOutput, ExtractParametersInput, ExtractParametersOutput, GenerateGraphInput,
    GenerateGraphOutput, GenerateTemplateInput, GenerateTemplateOutput, PlanInput, PlanOutput,
    PlanWithGraphInput, PlanWithGraphOutput, RequestClarificationInput, RequestClarificationOutput,
    SymbolValidationInput, SymbolValidationOutput, ValidatePlanInput, ValidatePlanOutput,
};

/// Central planning pipeline service that orchestrates the 6-phase flow.
///
/// The PlanningPipelineService sits between the user-facing interface
/// and the execution engine. It handles:
///
/// 1. **Budget Pre-check** — Ensures sufficient LLM capacity
/// 2. **Intent Classification** — Matches user intent to template via Classifier
/// 3. **Parameter Extraction** — Fills template parameters via ParameterExtractor
/// 4. **Graph Generation** — Produces TaskGraph via TemplateEngine
/// 5. **Plan Validation** — Validates via CompositeValidator
/// 6. **Hash Computation** — Deterministic hash for audit
///
/// # Lifecycle
///
/// 1. `plan` — End-to-end planning returning `PlanningResult`
/// 2. `plan_with_graph` — End-to-end planning including the generated TaskGraph
/// 3. `check_budget` — Standalone budget pre-check
/// 4. `classify_intent` — Standalone classification step
/// 5. `extract_parameters` — Standalone extraction step
/// 6. `generate_graph` — Standalone graph generation step
/// 7. `validate_plan` — Standalone validation step
///
/// # Cancellation Integration
///
/// The pipeline cooperates with the Cancellation module:
/// - Long-running LLM calls should check for cancellation signals
/// - State is preserved for graceful resumption after interruption
/// - Budget reservations are rolled back on cancellation
///
/// # Error Recovery
///
/// - Classification failures → retry with reduced template set
/// - Extraction failures → request clarification from user
/// - Generation failures → fallback to TemplateGenerator
/// - Validation failures → report errors, do not produce invalid plans
#[async_trait]
pub trait PlanningPipelineService: Send + Sync {
    /// Execute the full 6-phase planning flow and return a PlanningResult.
    ///
    /// This is the primary entry point. It:
    /// 1. Checks budget
    /// 2. Classifies intent against available templates
    /// 3. If confidence < 0.7, requests clarification or falls back to generator
    /// 4. Extracts parameters from intent
    /// 5. Generates TaskGraph via TemplateEngine
    /// 6. Validates the generated plan
    /// 7. Computes the deterministic planning_hash
    ///
    /// Returns a `PlanningResult` with the selected template, confidence,
    /// resolved parameters, and hash. Use `plan_with_graph` if you also
    /// need the generated TaskGraph.
    async fn plan(&self, input: PlanInput) -> Result<PlanOutput, PlanningError>;

    /// Execute the full 6-phase flow and return both the result and TaskGraph.
    ///
    /// Same as `plan()` but also returns the generated `TaskGraph` in
    /// a `PlanWithGraphOutput` wrapper. Use this when the caller needs the
    /// executable DAG (e.g., for immediate execution).
    async fn plan_with_graph(
        &self,
        input: PlanWithGraphInput,
    ) -> Result<PlanWithGraphOutput, PlanningError>;

    /// Run budget pre-check only.
    ///
    /// Checks if the available budget has capacity for at least 2 LLM calls
    /// (minimum required for classification + extraction). Returns the
    /// budget status without consuming any capacity.
    async fn check_budget(
        &self,
        input: CheckBudgetInput,
    ) -> Result<CheckBudgetOutput, PlanningError>;

    /// Classify user intent against available templates.
    ///
    /// Standalone classification step. Useful for UI previews or when
    /// the caller wants to inspect alternatives before proceeding.
    async fn classify_intent(
        &self,
        intent: UserIntent,
    ) -> Result<ClassificationResult, PlanningError>;

    /// Extract parameters for a selected template.
    ///
    /// Standalone extraction step. Useful when classification was done
    /// separately or the caller wants to re-extract with clarifications.
    async fn extract_parameters(
        &self,
        input: ExtractParametersInput,
    ) -> Result<ExtractParametersOutput, PlanningError>;

    /// Generate a TaskGraph from a template and its resolved parameters.
    ///
    /// Standalone graph generation step. Uses the TemplateEngine to
    /// produce an executable graph from the selected template.
    async fn generate_graph(
        &self,
        input: GenerateGraphInput,
    ) -> Result<GenerateGraphOutput, PlanningError>;

    /// Validate a generated plan/graph.
    ///
    /// Runs the CompositeValidator against the generated TaskGraph.
    /// Returns validation errors and warnings without modifying state.
    async fn validate_plan(
        &self,
        input: ValidatePlanInput,
    ) -> Result<ValidatePlanOutput, PlanningError>;

    /// Request clarification from the user for ambiguous intents.
    ///
    /// Called when classification confidence is between 0.3 and 0.7.
    /// Returns the question to ask the user. Call again with the
    /// user's response to re-classify.
    async fn request_clarification(
        &self,
        input: RequestClarificationInput,
    ) -> Result<RequestClarificationOutput, PlanningError>;

    /// Get the list of templates available for classification.
    ///
    /// Returns lightweight `TemplateSummary` metadata for all
    /// registered templates in the TemplateEngine.
    async fn available_templates(&self) -> Result<AvailableTemplatesOutput, PlanningError>;

    /// Get the execution ID for the current planning session.
    fn execution_id(&self) -> Uuid;
}

// ---------------------------------------------------------------------------
// TemplateGenerationService
// ---------------------------------------------------------------------------

/// Application service for generating templates from user intent.
///
/// Orchestrates the full template generation flow:
/// 1. Build RepoContext from the working directory
/// 2. Generate template via LLM with retry (up to 3 attempts)
/// 3. Validate generated TOML against schema
/// 4. Run Phase 3 symbol validation against indexed symbol graph
/// 5. Return validated template ready for registration
///
/// This service wraps the domain-level `TemplateGenerator` trait with
/// application concerns like retry logic, symbol validation integration,
/// and event emission.
///
/// # Contract (Frozen)
/// - All methods are async
/// - All public methods return `Result<_, PlanningError>`
/// - Input/output types are DTOs defined in `dto/`
/// - No implementation — only contract signatures
#[async_trait]
pub trait TemplateGenerationService: Send + Sync {
    /// Generate a template from user intent.
    ///
    /// Full flow: builds RepoContext, calls LLM generator with retry,
    /// validates parsed TOML, runs Phase 3 symbol validation.
    ///
    /// # Errors
    ///
    /// Returns `PlanningError::BudgetExhausted` if insufficient budget.
    /// Returns `PlanningError::TemplateEngineError` wrapping `GeneratorError`
    /// if generation fails after all retries.
    async fn generate_template(
        &self,
        input: GenerateTemplateInput,
    ) -> Result<GenerateTemplateOutput, PlanningError>;

    /// Build a RepoContext from a working directory.
    ///
    /// Scans the directory tree, detects project type, reads dependencies,
    /// and optionally indexes the symbol graph.
    async fn build_repo_context(
        &self,
        input: BuildRepoContextInput,
    ) -> Result<BuildRepoContextOutput, PlanningError>;

    /// Estimate the generation cost for a given intent.
    ///
    /// Returns the estimated number of LLM calls and tokens for
    /// budget pre-checking.
    async fn estimate_generation_cost(
        &self,
        intent: &UserIntent,
        repo_context: &RepoContext,
    ) -> Result<crate::template_generation::domain::GeneratedTemplateCost, PlanningError>;

    /// Generate a template and immediately register it in the TemplateEngine.
    ///
    /// Combines `generate_template()` + `TemplateEngineService::register()`
    /// for convenience. Returns the registered template metadata.
    async fn generate_and_register(
        &self,
        input: GenerateTemplateInput,
    ) -> Result<GenerateTemplateOutput, PlanningError>;
}

// ---------------------------------------------------------------------------
// SymbolValidationService
// ---------------------------------------------------------------------------

/// Application service for Phase 3 symbol validation.
///
/// Validates a generated template against the indexed symbol graph to
/// catch hallucinated type references, field accesses on non-existent
/// types, and `any` type usage (LLM escape hatch).
///
/// # Contract (Frozen)
/// - All methods are async
/// - Returns structured validation results with specific invalid references
/// - `flag_any_type` controls whether `any` type usage is treated as invalid
/// - No implementation — only contract signatures
#[async_trait]
pub trait SymbolValidationService: Send + Sync {
    /// Validate a template against the indexed symbol graph.
    ///
    /// Checks:
    /// - Type references in node actions exist in the symbol graph
    /// - Field access patterns (`var.field`) match actual type definitions
    /// - No hallucinated function/method calls
    /// - Optional `any` type detection
    async fn validate_template(
        &self,
        input: SymbolValidationInput,
    ) -> Result<SymbolValidationOutput, PlanningError>;

    /// Extract all symbol references from a template's nodes.
    ///
    /// Parses action fields for type names, field accesses, and
    /// function/method references. Returns the list of symbol names
    /// found for batch lookup against the symbol graph.
    async fn extract_symbol_references(
        &self,
        template: &crate::templates::domain::Template,
    ) -> Result<Vec<String>, PlanningError>;

    /// Get the current symbol graph snapshot for validation.
    ///
    /// Returns a JSON-serialized subset of the indexed symbol graph
    /// containing type definitions, field signatures, and function
    /// signatures relevant to the current workspace.
    async fn get_symbol_graph_snapshot(&self) -> Result<serde_json::Value, PlanningError>;

    /// Check if a symbol name exists in the indexed graph.
    ///
    /// Performs a case-sensitive exact match lookup.
    async fn symbol_exists(&self, name: &str) -> Result<bool, PlanningError>;
}
