//! Service interfaces (use cases) for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#services
//! Implements: Contract Freeze — ListTemplatesHandler, GetTemplateHandler,
//! CreateTemplateHandler, ValidateTemplateHandler service traits
//!
//! These traits define the application-level operations for template tools.
//! All methods are async and return domain error types. Input types are
//! domain value objects, output types are DTOs.
//!
//! # Contract (Frozen)
//!
//! - Every use case has a corresponding trait method
//! - Input types are domain value objects; output types are DTOs
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures
//! - Services are thread-safe (Send + Sync)

use async_trait::async_trait;

use crate::template_tools::domain::error::{HandlerError, ToolCallResult};
use crate::template_tools::domain::value::{
    CreateTemplateInput, GetTemplateInput, TemplateFilter, ValidateTemplateInput,
};

// ---------------------------------------------------------------------------
// ListTemplatesHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_list_templates` tool calls.
///
/// Discovers templates from filesystem via TemplateRepository and
/// formats the result as MCP tool call content.
///
/// # Contract (Frozen)
///
/// - Filter validation happens before repository delegate
/// - Results are formatted as ToolCallResult for MCP response
/// - Never panics — all errors go through HandlerError
#[async_trait]
pub trait ListTemplatesHandler: Send + Sync {
    /// Handle a `rigorix_list_templates` tool call.
    ///
    /// 1. Validates the filter criteria
    /// 2. Delegates to TemplateRepository for listing
    /// 3. Formats the template list as MCP tool call content
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if filter fails validation
    /// - `HandlerError::TemplateError` if TemplateRepository returns an error
    async fn handle(&self, filter: &TemplateFilter) -> Result<ToolCallResult, HandlerError>;
}

// ---------------------------------------------------------------------------
// GetTemplateHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_get_template` tool calls.
///
/// Reads a specific template from TemplateRepository and returns it
/// in the requested format (JSON or TOML).
///
/// # Contract (Frozen)
///
/// - Input validation happens before repository delegate
/// - Format defaults to JSON if not specified
/// - Results are formatted as ToolCallResult for MCP response
#[async_trait]
pub trait GetTemplateHandler: Send + Sync {
    /// Handle a `rigorix_get_template` tool call.
    ///
    /// 1. Validates the input (name, format)
    /// 2. Reads the template from TemplateRepository
    /// 3. Converts to requested format via TemplateConverter
    /// 4. Formats as MCP tool call content
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if input fails validation
    /// - `HandlerError::TemplateError` if TemplateRepository or Converter errors
    async fn handle(&self, input: &GetTemplateInput) -> Result<ToolCallResult, HandlerError>;
}

// ---------------------------------------------------------------------------
// CreateTemplateHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_create_template` tool calls.
///
/// Validates the input, checks for existing template (unless overwrite),
/// converts the plan via TemplateConverter, and persists through
/// TemplateRepository.
///
/// # Contract (Frozen)
///
/// - Name validation happens before repository check
/// - Overwrite guard prevents accidental replacement
/// - Conversion happens before persistence
/// - Results are formatted as ToolCallResult for MCP response
#[async_trait]
pub trait CreateTemplateHandler: Send + Sync {
    /// Handle a `rigorix_create_template` tool call.
    ///
    /// 1. Validates input (name, plan structure)
    /// 2. Checks if template exists (unless overwrite is true)
    /// 3. Validates and converts via TemplateConverter
    /// 4. Creates template via TemplateRepository
    /// 5. Formats result as MCP tool call content
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if input fails validation
    /// - `HandlerError::TemplateError` if TemplateRepository or Converter errors
    async fn handle(&self, input: &CreateTemplateInput) -> Result<ToolCallResult, HandlerError>;
}

// ---------------------------------------------------------------------------
// ValidateTemplateHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_validate_template` tool calls.
///
/// Validates template structure via TemplateConverter, then optionally
/// delegates to enforcement policies via EngineFacade.
///
/// # Contract (Frozen)
///
/// - Schema validation is always performed first
/// - Enforcement validation is optional (engine facade may not be available)
/// - Validation errors are wrapped, not propagated raw
/// - Results are formatted as ToolCallResult for MCP response
#[async_trait]
pub trait ValidateTemplateHandler: Send + Sync {
    /// Handle a `rigorix_validate_template` tool call.
    ///
    /// 1. Validates input schema via TemplateConverter
    /// 2. If schema passes, optionally delegates to EngineFacade for
    ///    enforcement policy validation
    /// 3. Formats validation result as MCP tool call content
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if input fails validation
    /// - `HandlerError::TemplateError` if Converter returns an error
    async fn handle(&self, input: &ValidateTemplateInput) -> Result<ToolCallResult, HandlerError>;
}
