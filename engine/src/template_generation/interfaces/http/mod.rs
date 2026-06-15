//! HTTP API contracts for Template Generation endpoints.
//!
//! @canonical .pi/architecture/modules/template-generation.md#http
//! Issue: issue-contract-freeze
//!
//! Template generation endpoints are served under the planning pipeline's
//! API base path at `/api/v1/planning/generate-template`.

/// API base path (shared with planning pipeline).
pub const API_BASE_PATH: &str = "/api/v1/planning";

/// POST /api/v1/planning/generate-template
pub const GENERATE_TEMPLATE_PATH: &str = "/api/v1/planning/generate-template";
pub const GENERATE_TEMPLATE_METHOD: &str = "POST";
