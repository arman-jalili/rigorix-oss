//! Boundary DTOs for the Sequence Policy module.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#ddd-layers
//! Implements: Contract Freeze — PlannedStep / DispatchedStep DTO schemas
//! Issue: #838 (sequence-policy epic — contract freeze)
//!
//! Typed step views fed to `SequencePolicyService::evaluate_plan` (R2,
//! plan-time) and `SequencePolicyService::evaluate_prefix` (R3, run-time
//! prefix gate). They are the *ordered serialized step data* the matcher is
//! deterministic over — derived from the same ordered step list the executor
//! will run.
//!
//! Mapping at the seams (implementation issues):
//! - R2: `orchestrator::TemplateStepDef` → `PlannedStep` (name, tool,
//!   parameters) before `build_graph_from_steps` seals the graph
//! - R3: completed `TaskNode`s → `DispatchedStep` prefix + next `PlannedStep`
//!   inside `run_dispatch_loop`
//!
//! # Contract (Frozen)
//! - Field names and types are frozen — implementation issues depend on them
//! - `name` is the step identity used across the engine (step names are what
//!   the approval chain addresses); `SequenceMatch.later_step` returns it
//! - `parameters` is the full JSON parameter object of the step — predicate
//!   JSON pointers resolve against it
//! - DTOs are serializable (JSON)

use serde::{Deserialize, Serialize};

/// A planned step in a fully-materialized ordered step list (plan-time, R2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedStep {
    /// Step name — the step identity the approval chain addresses and
    /// `SequenceMatch::later_step` returns.
    pub name: String,
    /// The tool/action this step dispatches (e.g. `"registration_remove"`).
    pub tool: String,
    /// Full JSON parameter object of the step. Predicate JSON pointers
    /// (e.g. `"/event_id"`) resolve against this object.
    pub parameters: serde_json::Value,
}

/// A step that already completed in the dispatch prefix (run-time, R3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchedStep {
    /// Step name (identity).
    pub name: String,
    /// The tool/action that was dispatched.
    pub tool: String,
    /// Full JSON parameter object of the dispatched step.
    pub parameters: serde_json::Value,
}
