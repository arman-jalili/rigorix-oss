//! HTTP API contracts for CLI Template Generation endpoints.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — HTTP endpoint contracts
//! Issue: issue-contract-freeze

use serde::{Deserialize, Serialize};

use crate::template_generation::application::dto::{
    CostEstimateOutput, DryRunOutput, GenerateOutput,
};

pub const API_BASE_PATH: &str = "/api/v1/cli/template-generation";

pub const GENERATE_PATH: &str = "/api/v1/cli/template-generation/generate";
pub const GENERATE_METHOD: &str = "POST";

pub const DRY_RUN_PATH: &str = "/api/v1/cli/template-generation/dry-run";
pub const DRY_RUN_METHOD: &str = "POST";

pub const ESTIMATE_COST_PATH: &str = "/api/v1/cli/template-generation/estimate-cost";
pub const ESTIMATE_COST_METHOD: &str = "POST";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateApiRequest {
    pub intent: String,
    #[serde(default)]
    pub stdout: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateApiResponse {
    pub template_id: String,
    pub saved_path: Option<String>,
    pub persisted: bool,
    pub content: String,
}

impl From<GenerateOutput> for GenerateApiResponse {
    fn from(o: GenerateOutput) -> Self {
        Self {
            template_id: o.template_id,
            saved_path: o.saved_path,
            persisted: o.persisted,
            content: o.content,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunApiRequest {
    pub intent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunApiResponse {
    pub template_id: String,
    pub content: String,
    pub valid: bool,
    pub errors: Vec<String>,
}

impl From<DryRunOutput> for DryRunApiResponse {
    fn from(o: DryRunOutput) -> Self {
        Self {
            template_id: o.template_id,
            content: o.content,
            valid: o.valid,
            errors: o.errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimateApiResponse {
    pub estimated_calls: u32,
    pub estimated_tokens: u32,
}

impl From<CostEstimateOutput> for CostEstimateApiResponse {
    fn from(o: CostEstimateOutput) -> Self {
        Self {
            estimated_calls: o.estimated_calls,
            estimated_tokens: o.estimated_tokens,
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
    pub const GENERATION_FAILED: &str = "TEMPLATE_GENERATION_FAILED";
    pub const BUDGET_EXCEEDED: &str = "TEMPLATE_GENERATION_BUDGET_EXCEEDED";
    pub const INTERNAL_ERROR: &str = "TEMPLATE_GENERATION_INTERNAL_ERROR";
}

pub mod status_codes {
    pub const GENERATION_FAILED: u16 = 422;
    pub const BUDGET_EXCEEDED: u16 = 402;
    pub const INTERNAL_ERROR: u16 = 500;
}
