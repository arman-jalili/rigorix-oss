//! Service interfaces (use cases) for the Failure Parser bounded context.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#service
//! Implements: Contract Freeze — FailureParserService trait
//! Issue: #495
//!
//! These traits define the application-level operations that can be performed
//! for failure parsing and suggested fix generation. All methods are
//! async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::failure_parser::domain::{
    TemplateFailure, SourceContext, FailureParserError, ParserRegistry,
};

use super::dto::{
    FormatForLlmInput, FormatForLlmOutput, ParseOutputInput, ParseOutputResult,
    RegisterParserInput, RegisterParserResult, SuggestFixInput, SuggestFixOutput,
};

/// Application service for parsing compiler/test output into structured failures.
///
/// The service orchestrates the parser registry to find the right parser
/// for the tool, delegates to it, and then generates suggested fixes
/// using the available source context.
///
/// # Flow
/// 1. Find appropriate parser in the ParserRegistry
/// 2. Parse raw output into ParsedFailure
/// 3. Generate suggested fixes for each failure using SourceContext
/// 4. Format results for LLM consumption
#[async_trait]
pub trait FailureParserService: Send + Sync {
    /// Parse raw compiler/test output into structured failures.
    ///
    /// Accepts the raw stdout/stderr from a tool execution and produces
    /// typed `TemplateFailure` values. Returns an empty vec if no failures
    /// are detected.
    ///
    /// # Arguments
    /// * `tool` - The tool that produced the output (e.g., "tsc", "jest", "rustc", "pytest")
    /// * `output` - The raw stdout/stderr
    /// * `source_context` - Available symbols and source code for suggestion generation
    async fn parse(
        &self,
        input: ParseOutputInput,
    ) -> Result<ParseOutputResult, FailureParserError>;

    /// Generate a human-readable summary suitable for LLM context.
    ///
    /// Produces a string like:
    /// ```text
    /// FAILURE ANALYSIS: 1 error found.
    ///  - TS2339 at tests/tasklist.test.ts:3:10: Property 'addTask' not found.
    ///    Available methods on TaskList: add, list, complete, count, activeCount.
    ///    Suggested fix: use 'add' instead of 'addTask'.
    /// ```
    async fn format_for_llm(
        &self,
        input: FormatForLlmInput,
    ) -> Result<FormatForLlmOutput, FailureParserError>;

    /// Generate a suggested fix for a single failure.
    ///
    /// Uses the source context to find similar symbols, check type signatures,
    /// and generate actionable fix suggestions.
    async fn suggest_fix(
        &self,
        input: SuggestFixInput,
    ) -> Result<SuggestFixOutput, FailureParserError>;

    /// Register a new parser in the parser registry.
    ///
    /// Allows runtime registration of custom parsers for new tools/languages.
    async fn register_parser(
        &self,
        input: RegisterParserInput,
    ) -> Result<RegisterParserResult, FailureParserError>;

    /// Get the underlying parser registry for inspection.
    fn parser_registry(&self) -> &ParserRegistry;
}

/// Trait for components that generate suggested fixes for failures.
///
/// This is the "Suggested Fix Generation" component referenced in the
/// epic. It can be implemented as a standalone service or integrated
/// into the FailureParserService.
#[async_trait]
pub trait FixSuggestionService: Send + Sync {
    /// Generate a suggested fix for a single TemplateFailure.
    ///
    /// Cross-references the failure against the available source context
    /// to produce actionable guidance.
    async fn suggest_fix(
        &self,
        failure: &TemplateFailure,
        source_context: &SourceContext,
    ) -> Result<Option<String>, FailureParserError>;

    /// Generate fixes for multiple failures at once (batched).
    ///
    /// More efficient than calling suggest_fix() individually when
    /// there are many failures, as it can cache symbol lookups.
    async fn suggest_fixes_batch(
        &self,
        failures: &[TemplateFailure],
        source_context: &SourceContext,
    ) -> Result<Vec<(usize, Option<String>)>, FailureParserError>;
}
