//! Event payload schemas for the Risk Gating bounded context.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: Contract Freeze — RiskGateEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted whenever a risk gate is evaluated — tools
//! classified, gates activated, overrides applied. Consumers (orchestrator,
//! audit, TUI) subscribe to these event types via the EventBus.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution

use serde::{Deserialize, Serialize};

use crate::risk_gating::domain::risk_level::RiskLevel;

/// Events emitted by the Risk Gating module.
///
/// Wrapped in `ExecutionEvent::risk_gate(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskGateEvent {
    /// A tool was classified into a risk level.
    ///
    /// Emitted for every classification, whether a default rule or
    /// a configured override was used.
    ToolClassified {
        /// The execution ID.
        execution_id: String,
        /// Identifier of the DAG node requesting the tool.
        node_id: String,
        /// The name of the tool being classified.
        tool: String,
        /// The risk level assigned.
        risk_level: RiskLevel,
        /// Whether this came from a configured override.
        from_override: bool,
        /// Human-readable reason for the classification.
        reason: String,
    },

    /// A risk gate was activated (confirmation or dry-run).
    ///
    /// Emitted when the gating policy requires user interaction
    /// (Medium → confirmation required) or dry-run mode (High).
    GateActivated {
        /// The execution ID.
        execution_id: String,
        /// Identifier of the DAG node.
        node_id: String,
        /// The name of the tool being gated.
        tool: String,
        /// The risk level that triggered the gate.
        risk_level: RiskLevel,
        /// The gating action taken.
        action: String,
    },

    /// A risk gate was resolved (approved or rejected).
    ///
    /// Emitted when a user responds to a confirmation request or
    /// when a dry-run is explicitly approved for execution.
    GateResolved {
        /// The execution ID.
        execution_id: String,
        /// Identifier of the DAG node.
        node_id: String,
        /// The name of the tool.
        tool: String,
        /// Whether the gate was approved (true) or rejected (false).
        approved: bool,
        /// The risk level that was gated.
        risk_level: RiskLevel,
    },

    /// A risk configuration was loaded or reloaded.
    ConfigLoaded {
        /// The execution ID.
        execution_id: String,
        /// Number of tool overrides defined.
        override_count: u32,
        /// Gating policy flags.
        auto_confirm_low: bool,
        require_review_medium: bool,
        dry_run_high: bool,
    },

    /// A risk configuration override was applied at runtime.
    OverrideApplied {
        /// The execution ID.
        execution_id: String,
        /// The tool name affected.
        tool: String,
        /// The new risk level.
        risk_level: RiskLevel,
        /// The previous risk level, if any.
        previous_level: Option<RiskLevel>,
    },
}
