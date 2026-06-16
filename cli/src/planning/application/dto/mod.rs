//! DTOs for the CLI Planning module.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — CLI planning DTO schemas
//! Issue: issue-contract-freeze

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanInput {
    pub intent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOutput {
    pub template_id: String,
    pub template_name: String,
    pub confidence: f64,
    pub nodes: Vec<PlanNodePreview>,
    pub total_estimated_tokens: u64,
    pub total_estimated_calls: u32,
    pub budget_exceeded: bool,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNodePreview {
    pub id: String,
    pub label: String,
    pub tool: String,
    pub depends_on: Vec<String>,
    pub estimated_tokens: Option<u64>,
    pub estimated_calls: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyInput {
    pub intent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyOutput {
    pub template_id: Option<String>,
    pub template_name: Option<String>,
    pub confidence: f64,
    pub alternatives: Vec<TemplateMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMatch {
    pub template_id: String,
    pub template_name: String,
    pub confidence: f64,
    pub description: String,
}
