//! Event payload schemas for the CLI Planning module.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — PlanningCliEvent
//! Issue: issue-contract-freeze

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanningCliEvent {
    PlanningStarted {
        intent: String,
    },
    PlanningCompleted {
        template_id: String,
        confidence: f64,
    },
    PlanningFailed {
        intent: String,
        error: String,
    },
    TemplateMatched {
        template_id: String,
        confidence: f64,
    },
    NoTemplateMatch {
        intent: String,
    },
    ClarificationRequested {
        question: String,
    },
}
