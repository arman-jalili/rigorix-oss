//! AuditQueryService — Aggregate Root interface for the Audit Tools bounded context.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#auditqueryservice
//! Implements: Contract Freeze — AuditQueryService trait, AuditFormatter trait
//!
//! The AuditQueryService is the aggregate root providing read-only access to
//! execution audit records from rigorix-engine. All three audit MCP tools
//! call through this interface.
//!
//! # Invariants (Frozen)
//!
//! - Read-only — never creates, modifies, or deletes audit data
//! - Always queries rigorix-engine directly (no local cache)
//! - Returns NotFound error for unknown execution IDs (not a panic)
//! - AuditEnvelope HMAC integrity is validated by rigorix-engine, not the gateway
//!
//! # Contract (Frozen)
//!
//! - All methods are async (use async-trait)
//! - All methods return Result with AuditError
//! - No implementation logic — pure interface
//! - Thread-safe (Send + Sync)

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

use super::error::AuditError;
use super::value::{AuditEnvelope, AuditFilter, AuditSummary};

use crate::execution_tools::domain::value::ExecutionId;

/// Read-only query interface for execution audit records from rigorix-engine.
///
/// All three audit MCP tools (read_audit, list_audits, audit_summary) call
/// through this aggregate root. Implementations wrap EngineFacade and/or
/// query rigorix-engine directly.
///
/// # Invariants (Frozen)
///
/// - Read-only — never creates, modifies, or deletes audit data
/// - Always queries rigorix-engine directly (no local cache)
/// - Returns NotFound error for unknown execution IDs (not a panic)
/// - AuditEnvelope HMAC integrity is validated by rigorix-engine, not the gateway
#[async_trait]
pub trait AuditQueryService: Send + Sync {
    /// Read a single audit record by execution ID.
    ///
    /// Returns the full audit envelope with all execution metadata,
    /// step results, token usage, and event history.
    ///
    /// # Errors
    /// - `AuditError::NotFound` if the execution ID does not exist
    /// - `AuditError::EngineNotAvailable` if rigorix-engine is unreachable
    async fn read_audit(&self, execution_id: &ExecutionId) -> Result<AuditEnvelope, AuditError>;

    /// List audit records matching the given filter criteria.
    ///
    /// Supports filtering by status, time range, and template name.
    /// Returns results ordered by completion time (newest first),
    /// limited by `AuditFilter.limit`.
    ///
    /// # Errors
    /// - `AuditError::InvalidFilter` if filter parameters are invalid
    /// - `AuditError::EngineNotAvailable` if rigorix-engine is unreachable
    async fn list_audits(&self, filter: AuditFilter) -> Result<Vec<AuditEnvelope>, AuditError>;

    /// Generate aggregate audit statistics over the given time range.
    ///
    /// Computes total executions, success/failure counts, success rate,
    /// total duration, token usage, top failures, and top templates.
    ///
    /// # Errors
    /// - `AuditError::InvalidFilter` if time range is invalid
    /// - `AuditError::EngineNotAvailable` if rigorix-engine is unreachable
    async fn audit_summary(
        &self,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<AuditSummary, AuditError>;
}

/// Shared ownership of an AuditQueryService implementation.
pub type SharedAuditQueryService = Arc<dyn AuditQueryService>;

// ---------------------------------------------------------------------------
// AuditFormatter — Domain Service
// ---------------------------------------------------------------------------

/// Formats audit data for MCP consumption: human-readable markdown or structured JSON.
///
/// All formatting methods are stateless — they take audit data by reference
/// and return formatted strings or JSON values.
///
/// # Contract (Frozen)
///
/// - All methods are pure functions (no side effects)
/// - Text format produces human-readable markdown
/// - JSON format produces structured serde_json::Value
/// - Panic-free — all inputs are valid by construction
pub trait AuditFormatter: Send + Sync {
    /// Format a single audit envelope as human-readable markdown text.
    fn format_audit_text(&self, envelope: &AuditEnvelope) -> String;

    /// Format a single audit envelope as structured JSON.
    fn format_audit_json(&self, envelope: &AuditEnvelope) -> serde_json::Value;

    /// Format a list of audit envelopes as human-readable markdown text.
    fn format_list_text(&self, audits: &[AuditEnvelope]) -> String;

    /// Format a list of audit envelopes as structured JSON.
    fn format_list_json(&self, audits: &[AuditEnvelope]) -> serde_json::Value;

    /// Format an audit summary as human-readable markdown text.
    fn format_summary_text(&self, summary: &AuditSummary) -> String;

    /// Format an audit summary as structured JSON.
    fn format_summary_json(&self, summary: &AuditSummary) -> serde_json::Value;
}
