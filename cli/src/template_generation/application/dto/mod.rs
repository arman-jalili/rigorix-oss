//! Data Transfer Objects for the CLI Template Generation module.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — CLI template generation DTO schemas
//! Issue: issue-contract-freeze

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateInput {
    pub intent: String,
    pub stdout: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateOutput {
    pub template_id: String,
    pub saved_path: Option<String>,
    pub persisted: bool,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunInput {
    pub intent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunOutput {
    pub template_id: String,
    pub content: String,
    pub valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimateInput {
    pub intent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimateOutput {
    pub estimated_calls: u32,
    pub estimated_tokens: u32,
}
