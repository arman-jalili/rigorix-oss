//! Scoring backend implementations — MCP, HTTP, and Local protocol adapters.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md
//! Implements: Contract Freeze — backend adapter interfaces
//! Issue: #673 (scored-evaluation epic)
//!
//! These files will implement the `ScoringBackend` trait (domain layer) for
//! three transport mechanisms:
//!
//! - `MCPBackend`: sends `rigorix_evaluate_artifact` MCP requests
//! - `HTTPBackend`: POSTs to a REST endpoint per the Rigorix scoring protocol
//! - `LocalBackend`: executes a local script per the Rigorix scoring protocol
//!
//! Rigorix defines the scoring protocol. External systems (e.g. RuntimeAI)
//! adopt it by implementing the server side of these protocols.

pub mod http_backend;
pub mod local_backend;
pub mod mcp_backend;

// Re-export backend types for convenience.
pub use http_backend::HttpBackend;
pub use local_backend::LocalBackend;
pub use mcp_backend::McpBackend;
