//! Factory interfaces for constructing Failure Parser service instances.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — ParserFactory, FailureParserServiceFactory traits
//! Issue: #495
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::failure_parser::domain::{FailureParserError, LanguageParser};

use super::service::{FailureParserService, FixSuggestionService};

/// Factory for constructing `LanguageParser` instances.
///
/// Implementations create and configure parser instances for specific
/// tools, injecting any required dependencies.
#[async_trait]
pub trait ParserFactory: Send + Sync {
    /// Create a TypeScript compiler parser.
    ///
    /// Parses `tsc --noEmit --pretty false` output format.
    async fn create_tsc_parser(&self) -> Result<Box<dyn LanguageParser>, FailureParserError>;

    /// Create a Jest test runner parser.
    ///
    /// Parses Jest test output (assertion errors, reference errors).
    async fn create_jest_parser(&self) -> Result<Box<dyn LanguageParser>, FailureParserError>;

    /// Create a Rust compiler parser.
    ///
    /// Parses `rustc --json=diagnostic` output.
    async fn create_rustc_parser(&self) -> Result<Box<dyn LanguageParser>, FailureParserError>;

    /// Create a Python test parser.
    ///
    /// Parses pytest output.
    async fn create_pytest_parser(&self) -> Result<Box<dyn LanguageParser>, FailureParserError>;

    /// Create all built-in parsers.
    ///
    /// Returns a vector of (tool_name, parser) pairs that can be
    /// registered in the ParserRegistry.
    async fn create_all_builtin(&self) -> Result<Vec<Box<dyn LanguageParser>>, FailureParserError>;
}

/// Factory for constructing `FailureParserService` instances.
///
/// Creates the main parser service with all built-in parsers registered.
#[async_trait]
pub trait FailureParserServiceFactory: Send + Sync {
    /// Create a fully configured FailureParserService.
    ///
    /// Registers all built-in parsers and returns a ready-to-use
    /// service instance.
    async fn create_service(&self) -> Result<Box<dyn FailureParserService>, FailureParserError>;

    /// Create a FailureParserService with additional custom parsers.
    async fn create_service_with_custom_parsers(
        &self,
        custom_parsers: Vec<Box<dyn LanguageParser>>,
    ) -> Result<Box<dyn FailureParserService>, FailureParserError>;
}

/// Factory for constructing `FixSuggestionService` instances.
#[async_trait]
pub trait FixSuggestionServiceFactory: Send + Sync {
    /// Create a FixSuggestionService instance.
    async fn create(&self) -> Result<Box<dyn FixSuggestionService>, FailureParserError>;
}
