//! Service interfaces (use cases) for the Audit Tools bounded context.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#services
//! Implements: Contract Freeze — ReadAuditHandler, ListAuditsHandler,
//! AuditSummaryHandler service traits
//!
//! These traits define the application-level operations for audit tools.
//! All methods are async and return domain error types. Input/output types
//! are DTOs defined in `dto/`.
//!
//! # Contract (Frozen)
//!
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures
//! - Services are thread-safe (Send + Sync)

use async_trait::async_trait;

use crate::audit_tools::domain::error::AuditHandlerError;
use crate::execution_tools::domain::error::ToolCallResult;

use super::dto::{AuditSummaryInput, AuthorityFactsInput, ListAuditsInput, ReadAuditInput};

// ---------------------------------------------------------------------------
// ReadAuditHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_read_audit` tool calls.
///
/// Validates the execution ID, delegates to AuditQueryService,
/// and formats the audit envelope as MCP tool call content (text or JSON).
///
/// # Contract (Frozen)
///
/// - Input validation (execution_id format) happens before service delegate
/// - Supports "text" and "json" output formats
/// - Text format defaults to human-readable markdown
/// - Never panics — all errors go through AuditHandlerError
#[async_trait]
pub trait ReadAuditHandler: Send + Sync {
    /// Handle a `rigorix_read_audit` tool call.
    ///
    /// 1. Validates the input (execution_id format)
    /// 2. Queries AuditQueryService for the audit record
    /// 3. Formats the result as text or JSON based on `format` field
    /// 4. Returns MCP tool call result content
    ///
    /// # Errors
    /// - `AuditHandlerError::InvalidArguments` if execution_id format is invalid
    /// - `AuditHandlerError::AuditError` if AuditQueryService returns an error
    async fn handle(&self, input: ReadAuditInput) -> Result<ToolCallResult, AuditHandlerError>;
}

// ---------------------------------------------------------------------------
// ListAuditsHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_list_audits` tool calls.
///
/// Parses filter parameters from input, delegates to AuditQueryService,
/// and formats the results as MCP tool call content.
///
/// # Contract (Frozen)
///
/// - Filter parameters are optional — unset fields are not filtered
/// - Default limit is 50 results
/// - Results are ordered by completion time (newest first)
/// - Never panics — all errors go through AuditHandlerError
#[async_trait]
pub trait ListAuditsHandler: Send + Sync {
    /// Handle a `rigorix_list_audits` tool call.
    ///
    /// 1. Validates and parses filter parameters from input
    /// 2. Builds an AuditFilter from the parsed parameters
    /// 3. Queries AuditQueryService for matching records
    /// 4. Formats the results as MCP tool call content
    ///
    /// # Errors
    /// - `AuditHandlerError::InvalidArguments` if filter parameters are invalid
    /// - `AuditHandlerError::AuditError` if AuditQueryService returns an error
    async fn handle(&self, input: ListAuditsInput) -> Result<ToolCallResult, AuditHandlerError>;
}

// ---------------------------------------------------------------------------
// AuditSummaryHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_audit_summary` tool calls.
///
/// Parses time range from input, delegates to AuditQueryService,
/// and formats the aggregate statistics as MCP tool call content.
///
/// # Contract (Frozen)
///
/// - Default time range is last 7 days (if `since` not specified)
/// - `until` defaults to current time if not specified
/// - Success rate is computed as success_count / total_executions
/// - Never panics — all errors go through AuditHandlerError
#[async_trait]
pub trait AuditSummaryHandler: Send + Sync {
    /// Handle a `rigorix_audit_summary` tool call.
    ///
    /// 1. Validates and parses time range from input
    /// 2. Applies defaults (since = 7 days ago, until = now)
    /// 3. Queries AuditQueryService for aggregate data
    /// 4. Formats the summary as MCP tool call content
    ///
    /// # Errors
    /// - `AuditHandlerError::InvalidArguments` if time range is invalid
    /// - `AuditHandlerError::AuditError` if AuditQueryService returns an error
    async fn handle(&self, input: AuditSummaryInput) -> Result<ToolCallResult, AuditHandlerError>;
}

// ---------------------------------------------------------------------------
// AuditAuthorityFactsHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_audit_authority_facts` tool calls.
///
/// Projects the signed audit history into an authority-fact bundle a third party
/// can verify by **re-derivation** (spec §4.3.1) — not by trusting a signature.
///
/// # Contract (Frozen)
///
/// - The bundle carries the signed history: the caller receives the `envelopes`,
///   not a summary. That is inherent to re-derivation, and it is disclosed in the
///   tool description rather than left to be discovered.
/// - `subjects_without_record` names only the requested subjects that have **no**
///   record. A subject that has one MUST NOT appear there: the verifier re-derives
///   with that list verbatim, so the bundle would self-verify while asserting
///   `NOT_ESTABLISHED` over an established fact.
/// - The projection is the ENGINE's; this layer never re-derives.
/// - Never panics — all errors go through AuditHandlerError
#[async_trait]
pub trait AuditAuthorityFactsHandler: Send + Sync {
    /// Handle a `rigorix_audit_authority_facts` tool call.
    ///
    /// 1. Reads the whole signed history (a bundle must carry a complete chain)
    /// 2. Applies the optional `producer_id` scope
    /// 3. Projects via the engine and computes `subjects_without_record`
    /// 4. Returns the bundle as MCP tool call content
    ///
    /// # Errors
    /// - `AuditHandlerError::InvalidArguments` if `subjects` is empty
    /// - `AuditHandlerError::Internal` if the history cannot be read or the
    ///   bundle cannot be serialized
    async fn handle(&self, input: AuthorityFactsInput)
    -> Result<ToolCallResult, AuditHandlerError>;
}
