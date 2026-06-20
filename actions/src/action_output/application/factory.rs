//! Factory interfaces for constructing Action Output domain objects.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — OutputFactory, AnnotationFactory, SummaryFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject formatting logic and apply defaults
//! without exposing construction details to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::action_output::domain::{
    ActionOutputError, ExecutionContext, FormattedOutput, OutputVariable, StepSummary,
    WorkflowAnnotation,
};

/// Factory for constructing `FormattedOutput` from various engine result types.
///
/// Implementations handle the conversion from engine types (RunOutput,
/// ValidationReport, TemplateFailure) into the formatted output domain types.
///
/// # Contract (Frozen)
/// - All formatting is stateless
/// - Engine-specific type knowledge is hidden behind this factory
/// - Output is always a valid `FormattedOutput` (may be empty)
#[async_trait]
pub trait OutputFactory: Send + Sync {
    /// Build a `FormattedOutput` from an execution context.
    ///
    /// This is the primary factory method. It produces the full
    /// output bundle (summary, annotations, variables, PR comment body)
    /// from a single execution context.
    async fn build_from_context(
        &self,
        context: &ExecutionContext,
        include_details: bool,
        post_pr_comment: bool,
    ) -> Result<FormattedOutput, ActionOutputError>;

    /// Build a `FormattedOutput` for validation failure output.
    ///
    /// Produces annotations for each failure and a detailed step summary
    /// showing which validations passed/failed.
    async fn build_validation_failure_output(
        &self,
        context: &ExecutionContext,
        annotations: &[WorkflowAnnotation],
        execution_id: &uuid::Uuid,
    ) -> Result<FormattedOutput, ActionOutputError>;

    /// Create an empty output with just the execution status variable.
    async fn empty_output(&self, status: &str) -> FormattedOutput;
}

/// Factory for constructing `WorkflowAnnotation` from failure descriptions.
///
/// Handles mapping from various failure types to structured annotations
/// with proper file/line/column positioning.
///
/// # Contract (Frozen)
/// - Every failure type has a matching annotation format
/// - Unknown failure types produce a generic annotation
/// - File paths are normalized to repo-relative
#[async_trait]
pub trait AnnotationFactory: Send + Sync {
    /// Build a `WorkflowAnnotation` from a generic failure description.
    async fn build_from_failure(
        &self,
        failure_type: &str,
        context: &str,
        file: Option<&str>,
        line: Option<usize>,
        details: Option<serde_json::Value>,
    ) -> Result<WorkflowAnnotation, ActionOutputError>;

    /// Build a `WorkflowAnnotation` for a missing symbol failure.
    async fn build_missing_symbol(
        &self,
        symbol: &str,
        file: &str,
        line: usize,
        suggestion: Option<&str>,
    ) -> Result<WorkflowAnnotation, ActionOutputError>;

    /// Build a `WorkflowAnnotation` for a type mismatch failure.
    async fn build_type_mismatch(
        &self,
        expected: &str,
        received: &str,
        file: &str,
        line: usize,
    ) -> Result<WorkflowAnnotation, ActionOutputError>;

    /// Build a `WorkflowAnnotation` for a compile error.
    async fn build_compile_error(
        &self,
        code: &str,
        message: &str,
        file: &str,
        line: usize,
    ) -> Result<WorkflowAnnotation, ActionOutputError>;

    /// Build a `WorkflowAnnotation` for a test assertion failure.
    async fn build_assertion_failure(
        &self,
        test_name: &str,
        expected: &str,
        received: &str,
        file: &str,
        line: usize,
    ) -> Result<WorkflowAnnotation, ActionOutputError>;

    /// Build a generic error annotation with no file location.
    async fn build_generic_error(
        &self,
        title: &str,
        message: &str,
    ) -> Result<WorkflowAnnotation, ActionOutputError>;
}

/// Factory for constructing `StepSummary` from execution data.
///
/// Handles the markdown formatting of execution plans, validation reports,
/// and status updates into structured summary sections.
///
/// # Contract (Frozen)
/// - All methods produce valid markdown content
/// - Large content blocks are wrapped in collapsible `<details>` sections
/// - Sensitive data is never included in summaries
#[async_trait]
pub trait SummaryFactory: Send + Sync {
    /// Build a `StepSummary` from an execution context.
    async fn build_from_context(
        &self,
        context: &ExecutionContext,
    ) -> Result<StepSummary, ActionOutputError>;

    /// Build a validation report summary from execution context and annotations.
    async fn build_validation_report(
        &self,
        context: &ExecutionContext,
        annotations: &[WorkflowAnnotation],
    ) -> Result<StepSummary, ActionOutputError>;

    /// Build a quick status summary (minimal — just status + key metrics).
    async fn build_status_summary(
        &self,
        status: &str,
        execution_id: &uuid::Uuid,
        duration_ms: u64,
        quality_level: Option<&str>,
    ) -> Result<StepSummary, ActionOutputError>;
}

/// Factory for constructing `OutputVariable` instances.
///
/// Encapsulates variable name validation and value sanitization.
///
/// # Contract (Frozen)
/// - Variable names are validated against `[a-z_][a-z0-9_]*`
/// - Values are sanitized (newlines stripped, length capped)
/// - Standard variable names are used for well-known outputs
#[async_trait]
pub trait OutputVariableFactory: Send + Sync {
    /// Build output variables from an execution context.
    ///
    /// Produces the standard set of output variables:
    /// execution_id, status, iterations, template_id, quality_level,
    /// failure_count, cumulative_tokens, duration_ms.
    async fn build_from_context(
        &self,
        context: &ExecutionContext,
    ) -> Result<Vec<OutputVariable>, ActionOutputError>;

    /// Build a single output variable with validation.
    async fn build_variable(
        &self,
        name: &str,
        value: &str,
    ) -> Result<OutputVariable, ActionOutputError>;

    /// Validate a variable name.
    fn validate_name(&self, name: &str) -> bool;

    /// Sanitize a variable value (strip newlines, cap length).
    fn sanitize_value(&self, value: &str, max_length: usize) -> String;
}
