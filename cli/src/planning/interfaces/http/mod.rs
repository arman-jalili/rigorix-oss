//! HTTP API contracts for CLI Planning endpoints.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — HTTP endpoint contracts
//! Issue: issue-contract-freeze

use serde::{Deserialize, Serialize};

use crate::planning::application::dto::{ClassifyOutput, PlanOutput};

pub const API_BASE_PATH: &str = "/api/v1/cli/planning";

pub const PLAN_PATH: &str = "/api/v1/cli/planning/plan";
pub const PLAN_METHOD: &str = "POST";

pub const CLASSIFY_PATH: &str = "/api/v1/cli/planning/classify";
pub const CLASSIFY_METHOD: &str = "POST";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanApiRequest {
    pub intent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanApiResponse {
    pub template_id: String,
    pub template_name: String,
    pub confidence: f64,
    pub is_valid: bool,
    pub budget_exceeded: bool,
    pub node_count: u32,
}

impl From<PlanOutput> for PlanApiResponse {
    fn from(o: PlanOutput) -> Self {
        Self {
            template_id: o.template_id,
            template_name: o.template_name,
            confidence: o.confidence,
            is_valid: o.is_valid,
            budget_exceeded: o.budget_exceeded,
            node_count: o.nodes.len() as u32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyApiRequest {
    pub intent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyApiResponse {
    pub template_id: Option<String>,
    pub confidence: f64,
    pub alternatives: Vec<ClassifyAlternative>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyAlternative {
    pub template_id: String,
    pub template_name: String,
    pub confidence: f64,
}

impl From<ClassifyOutput> for ClassifyApiResponse {
    fn from(o: ClassifyOutput) -> Self {
        Self {
            template_id: o.template_id,
            confidence: o.confidence,
            alternatives: o
                .alternatives
                .into_iter()
                .map(|a| ClassifyAlternative {
                    template_id: a.template_id,
                    template_name: a.template_name,
                    confidence: a.confidence,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliApiErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
}

pub mod error_codes {
    pub const PLANNING_FAILED: &str = "PLANNING_FAILED";
    pub const NO_TEMPLATE_MATCH: &str = "PLANNING_NO_TEMPLATE_MATCH";
    pub const BUDGET_EXCEEDED: &str = "PLANNING_BUDGET_EXCEEDED";
    pub const INTERNAL_ERROR: &str = "PLANNING_INTERNAL_ERROR";
}

pub mod status_codes {
    pub const PLANNING_FAILED: u16 = 422;
    pub const NO_TEMPLATE_MATCH: u16 = 404;
    pub const BUDGET_EXCEEDED: u16 = 402;
    pub const INTERNAL_ERROR: u16 = 500;
}
