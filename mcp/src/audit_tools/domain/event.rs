//! Domain events for the Audit Tools bounded context.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#events
//! Implements: Contract Freeze — audit-tools event payload schemas
//!
//! These events are emitted by audit tool handlers when audit operations are
//! performed. Consumers (observability, telemetry, audit trail correlation)
//! subscribe to these event types.
//!
//! # Event Catalog
//!
//! | Event | Trigger | Published By |
//! |-------|---------|-------------|
//! | AuditRead | ReadAuditHandler after successful read | ReadAuditHandler |
//! | AuditListed | ListAuditsHandler after successful list | ListAuditsHandler |
//! | AuditSummarized | AuditSummaryHandler after successful summary | AuditSummaryHandler |
//!
//! # Contract (Frozen)
//!
//! - Every event carries an execution_id or session_id and timestamp for correlation
//! - Serialized as tagged union with `#[serde(tag = "type")]`
//! - Events are facts — immutable and append-only
//! - No behavior — pure data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// All domain events emitted by the Audit Tools bounded context.
///
/// Each variant represents a meaningful domain occurrence.
/// Consumers use these events for observability, logging, and metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditToolsEvent {
    /// An audit record was read via `rigorix_read_audit`.
    AuditRead {
        /// Unique execution identifier that was queried.
        execution_id: Uuid,
        /// Requested output format ("text" or "json").
        format: String,
        /// Timestamp of the read operation.
        timestamp: DateTime<Utc>,
    },

    /// Audit records were listed via `rigorix_list_audits`.
    AuditListed {
        /// Session identifier.
        session_id: Uuid,
        /// Number of filter criteria applied.
        filter_count: usize,
        /// Number of results returned.
        result_count: usize,
        /// Timestamp of the listing.
        timestamp: DateTime<Utc>,
    },

    /// An audit summary was generated via `rigorix_audit_summary`.
    AuditSummarized {
        /// Session identifier.
        session_id: Uuid,
        /// Start of the time range.
        since: DateTime<Utc>,
        /// End of the time range.
        until: DateTime<Utc>,
        /// Total executions in the range.
        total_executions: u64,
        /// Overall success rate (0.0 to 1.0).
        success_rate: f64,
        /// Timestamp of the summary generation.
        timestamp: DateTime<Utc>,
    },
}
