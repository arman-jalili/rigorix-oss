//! PlanningResult, PlanningHash, and PlanOutput domain entities.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#result
//! Implements: Contract Freeze — PlanningResult deterministic contract, PlanningHash value object
//! Issue: issue-contract-freeze
//!
//! Defines the deterministic contract that the planning pipeline produces:
//! - `PlanningResult`: The core output contract carrying template ID, confidence,
//!   resolved parameters, and the deterministic planning_hash.
//! - `PlanningHash`: A SHA-256 based value object for deterministic replay auditing.
//! - `PlanOutput`: Extended result that includes the generated TaskGraph.
//! - `TemplateSummary`: Lightweight template metadata for `available_templates()`.
//!
//! # Determinism
//!
//! The `planning_hash` is a SHA-256 of (template_id + sorted_parameters + intent_input).
//! This ensures that the same intent and template always produce the same hash,
//! enabling deterministic replay and audit verification.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::dag_engine::domain::TaskGraph;

// ---------------------------------------------------------------------------
// PlanningHash — Deterministic Replay Identifier
// ---------------------------------------------------------------------------

/// A SHA-256 based hash that uniquely identifies a planning outcome.
///
/// Computed deterministically from (template_id + sorted_parameters + intent_input)
/// to enable replay auditing. The same inputs always produce the same hash.
///
/// # Determinism Guarantee
///
/// - Same template_id + parameters + intent → same PlanningHash
/// - Order of parameters is normalised (sorted by key) before hashing
/// - The hash is hex-encoded (64 hex characters)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanningHash(pub String);

impl PlanningHash {
    /// Create a new PlanningHash from a hex-encoded SHA-256 digest.
    ///
    /// # Panics
    ///
    /// Panics if `hash` is not exactly 64 hex characters.
    pub fn new(hash: String) -> Self {
        assert_eq!(
            hash.len(),
            64,
            "PlanningHash must be exactly 64 hex characters (SHA-256)"
        );
        Self(hash)
    }

    /// Return the inner hash string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// PlanningResult — Core Output Contract
// ---------------------------------------------------------------------------

/// The deterministic output of the planning phase.
///
/// Carries everything needed to execute a plan: template selection, confidence,
/// resolved parameters, timing, and the deterministic hash for replay auditing.
///
/// This is the primary contract that downstream consumers (execution engine,
/// audit system) depend on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningResult {
    /// The execution ID this plan belongs to.
    pub execution_id: Uuid,

    /// The template that was selected for this execution.
    pub template_id: String,

    /// Model confidence score (0.0–1.0) for the generated plan.
    pub confidence: f64,

    /// Key-value pairs of resolved template parameters.
    pub parameters: HashMap<String, String>,

    /// Deterministic planning hash for audit replay verification.
    pub planning_hash: PlanningHash,

    /// Whether the classifier required clarification to reach this result.
    pub required_clarification: bool,

    /// ISO 8601 timestamp when planning completed.
    pub planned_at: DateTime<Utc>,

    /// Number of LLM calls consumed during planning.
    pub llm_calls_used: u32,

    /// Number of LLM tokens consumed during planning.
    pub llm_tokens_used: u32,
}

impl PlanningResult {
    /// Create a new PlanningResult.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        execution_id: Uuid,
        template_id: String,
        confidence: f64,
        parameters: HashMap<String, String>,
        planning_hash: PlanningHash,
        required_clarification: bool,
        llm_calls_used: u32,
        llm_tokens_used: u32,
    ) -> Self {
        Self {
            execution_id,
            template_id,
            confidence,
            parameters,
            planning_hash,
            required_clarification,
            planned_at: Utc::now(),
            llm_calls_used,
            llm_tokens_used,
        }
    }
}

// ---------------------------------------------------------------------------
// PlanOutput — Extended Result with TaskGraph
// ---------------------------------------------------------------------------

/// Extended planning output that includes the generated TaskGraph.
///
/// Used by `plan_with_graph()` when the caller needs both the planning
/// result and the executable DAG. The TaskGraph is a sealed, validated
/// graph ready for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOutput {
    /// The core planning result.
    pub planning_result: PlanningResult,

    /// The generated TaskGraph (sealed and validated).
    pub graph: TaskGraph,

    /// Number of nodes in the generated graph.
    pub node_count: u32,

    /// Whether the graph passed all validation checks.
    pub validation_passed: bool,

    /// List of validation warnings (non-blocking).
    #[serde(default)]
    pub validation_warnings: Vec<String>,
}

impl PlanOutput {
    /// Create a new PlanOutput from a PlanningResult and TaskGraph.
    pub fn new(
        planning_result: PlanningResult,
        graph: TaskGraph,
        validation_warnings: Vec<String>,
    ) -> Self {
        let node_count = graph.nodes.len() as u32;
        Self {
            planning_result,
            graph,
            node_count,
            validation_passed: validation_warnings.is_empty(),
            validation_warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// TemplateSummary — Lightweight Template Metadata
// ---------------------------------------------------------------------------

/// Lightweight metadata about a registered template.
///
/// Returned by `available_templates()` for callers to inspect what
/// templates are available without loading full definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSummary {
    /// The template's unique identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Short description of the template's purpose.
    pub description: String,
    /// Number of parameters this template accepts.
    pub parameter_count: u32,
    /// Number of nodes in this template's DAG.
    pub node_count: u32,
    /// Optional category for grouping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}
