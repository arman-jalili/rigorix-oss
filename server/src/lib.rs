//! `rigorix-server` — OSS reference execution host (#888 OSS-C2).
//!
//! Exposes the frozen `rigorix.*` catalog (ADR-0001) over HTTP:
//! JSON-RPC 2.0 at `POST /rpc`. This is a **host over the existing engine**:
//! every method delegates to the same composed host the stdio MCP binary uses
//! (`rigorix_mcp::host::AppState::handle_tool_call`), so method behavior is
//! identical to the MCP tool counterparts (same DTOs, same error taxonomy).
//!
//! This crate is deliberately thin: the catalog is the contract.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`catalog`] | The frozen method table (name → auth level → MCP tool) |
//! | [`backend`] | [`MethodBackend`] seam; [`HostBackend`] delegates to `rigorix-mcp` |
//! | [`rpc`] | JSON-RPC 2.0 codec: single, batch, notification; errors |
//! | [`version`] | `rigorix.system.version` result |

pub mod backend;
pub mod catalog;
pub mod rpc;
pub mod version;

pub use backend::{HostBackend, MethodBackend};
pub use catalog::{AuthLevel, CATALOG, CatalogEntry};
