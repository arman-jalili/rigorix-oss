//! HTTP API contracts for CLI Boundary endpoints.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats for top-level CLI operations (run, plan, generate, init,
//! history, logs, audit, template list/show).
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::cli_boundary::application::dto::{
    GenerateOutput, HistoryListOutput, HistoryShowOutput, InitOutput, PlanOutput, RunOutput,
    TemplateListOutput, TemplateShowOutput,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

pub const API_BASE_PATH: &str = "/api/v1/cli";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/cli/run
// ---------------------------------------------------------------------------

pub const RUN_PATH: &str = "/api/v1/cli/run";
pub const RUN_METHOD: &str = "POST";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunApiRequest {
    pub intent: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub skip_confirmations: bool,
    #[serde(default)]
    pub skip_budget_check: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunApiResponse {
    pub session_id: String,
    pub outcome: String,
    pub total_nodes: u32,
    pub completed: u32,
    pub failed: u32,
}

impl From<RunOutput> for RunApiResponse {
    fn from(o: RunOutput) -> Self {
        Self {
            session_id: o.session_id,
            outcome: o.outcome.to_string(),
            total_nodes: o.summary.total_nodes,
            completed: o.summary.completed,
            failed: o.summary.failed,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/cli/plan
// ---------------------------------------------------------------------------

pub const PLAN_PATH: &str = "/api/v1/cli/plan";
pub const PLAN_METHOD: &str = "POST";

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
}

impl From<PlanOutput> for PlanApiResponse {
    fn from(o: PlanOutput) -> Self {
        Self {
            template_id: o.template_id,
            template_name: o.template_name,
            confidence: o.confidence,
            is_valid: o.is_valid,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/cli/generate
// ---------------------------------------------------------------------------

pub const GENERATE_PATH: &str = "/api/v1/cli/generate";
pub const GENERATE_METHOD: &str = "POST";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateApiRequest {
    pub intent: String,
    #[serde(default)]
    pub stdout: bool,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateApiResponse {
    pub template_id: String,
    pub persisted: bool,
    pub content: String,
}

impl From<GenerateOutput> for GenerateApiResponse {
    fn from(o: GenerateOutput) -> Self {
        Self {
            template_id: o.template_id,
            persisted: o.persisted,
            content: o.content,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/cli/init
// ---------------------------------------------------------------------------

pub const INIT_PATH: &str = "/api/v1/cli/init";
pub const INIT_METHOD: &str = "POST";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitApiRequest {
    pub target_path: String,
    #[serde(default)]
    pub interactive: bool,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitApiResponse {
    pub created_path: String,
    pub files_created: Vec<String>,
    pub api_key_configured: bool,
}

impl From<InitOutput> for InitApiResponse {
    fn from(o: InitOutput) -> Self {
        Self {
            created_path: o.created_path,
            files_created: o.files_created,
            api_key_configured: o.api_key_configured,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/cli/history
// ---------------------------------------------------------------------------

pub const HISTORY_LIST_PATH: &str = "/api/v1/cli/history";
pub const HISTORY_LIST_METHOD: &str = "GET";

pub const HISTORY_SHOW_PATH: &str = "/api/v1/cli/history/{id}";
pub const HISTORY_SHOW_METHOD: &str = "GET";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryListApiResponse {
    pub sessions: Vec<HistorySessionItem>,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySessionItem {
    pub session_id: String,
    pub command: String,
    pub outcome: String,
    pub duration_ms: u64,
}

impl From<HistoryListOutput> for HistoryListApiResponse {
    fn from(o: HistoryListOutput) -> Self {
        Self {
            sessions: o
                .sessions
                .into_iter()
                .map(|s| HistorySessionItem {
                    session_id: s.session_id,
                    command: s.command,
                    outcome: s.outcome.to_string(),
                    duration_ms: s.duration_ms,
                })
                .collect(),
            total: o.total,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryShowApiResponse {
    pub session: HistorySessionItem,
}

impl From<HistoryShowOutput> for HistoryShowApiResponse {
    fn from(o: HistoryShowOutput) -> Self {
        Self {
            session: HistorySessionItem {
                session_id: o.session.session_id,
                command: o.session.command,
                outcome: o.session.outcome.to_string(),
                duration_ms: o.session.duration_ms,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/cli/templates
// ---------------------------------------------------------------------------

pub const TEMPLATE_LIST_PATH: &str = "/api/v1/cli/templates";
pub const TEMPLATE_LIST_METHOD: &str = "GET";

pub const TEMPLATE_SHOW_PATH: &str = "/api/v1/cli/templates/{id}";
pub const TEMPLATE_SHOW_METHOD: &str = "GET";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateListApiResponse {
    pub templates: Vec<TemplateItem>,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub built_in: bool,
}

impl From<TemplateListOutput> for TemplateListApiResponse {
    fn from(o: TemplateListOutput) -> Self {
        Self {
            templates: o
                .templates
                .into_iter()
                .map(|t| TemplateItem {
                    id: t.id,
                    name: t.name,
                    description: t.description,
                    built_in: t.built_in,
                })
                .collect(),
            total: o.total,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateShowApiResponse {
    pub content: String,
}

impl From<TemplateShowOutput> for TemplateShowApiResponse {
    fn from(o: TemplateShowOutput) -> Self {
        Self { content: o.content }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliApiErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
}

pub mod error_codes {
    pub const CONFIG_NOT_FOUND: &str = "CLI_CONFIG_NOT_FOUND";
    pub const ENGINE_ERROR: &str = "CLI_ENGINE_ERROR";
    pub const VALIDATION_ERROR: &str = "CLI_VALIDATION_ERROR";
    pub const INTERNAL_ERROR: &str = "CLI_INTERNAL_ERROR";
}

pub mod status_codes {
    pub const CONFIG_NOT_FOUND: u16 = 404;
    pub const ENGINE_ERROR: u16 = 502;
    pub const VALIDATION_ERROR: u16 = 422;
    pub const INTERNAL_ERROR: u16 = 500;
}
