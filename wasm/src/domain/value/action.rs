//! `GatedAction` — the proposed action under evaluation.
//!
//! @canonical .pi/architecture/modules/portable-session.md#aggregates
//! Implements: Contract Freeze — GatedAction, Params
//! Issue: #1294 (contract freeze)
//!
//! The action is a proposal, not a business transaction: the crate records what was
//! proposed and what the gate decided about it, and never performs the action.

use serde::{Deserialize, Serialize};

/// Action parameters, exactly as the host supplied them.
///
/// # Contract (Frozen)
/// - A JSON object; key order is canonicalised by the engine's serializer
/// - Values are data the policy evaluates, never authority (FR-009)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Params(pub serde_json::Map<String, serde_json::Value>);

/// One proposed action submitted to the gate.
///
/// # Contract (Frozen)
/// - `name` is matched by the engine's `StepPredicate`; this crate never matches it
/// - Carries no verdict, no approval grant and no override (FR-009)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatedAction {
    /// The tool/step name the scenario's predicates match on.
    pub name: String,
    /// Parameters the policy may inspect.
    pub params: Params,
}

impl GatedAction {
    /// Propose an action.
    pub fn new(name: impl Into<String>, params: Params) -> Self {
        Self {
            name: name.into(),
            params,
        }
    }
}
