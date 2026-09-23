//! Method backend seam (#888 OSS-C2).
//!
//! [`MethodBackend`] decouples the JSON-RPC transport from the engine so the
//! transport/catalog/auth rules are unit-testable without a live engine.
//! [`HostBackend`] is the production implementation: it delegates every method
//! to the shared composed host in `rigorix-mcp` (the SAME dispatcher the stdio
//! MCP binary uses), guaranteeing behavioral parity.

use async_trait::async_trait;
use serde_json::Value;

use rigorix_mcp::host::app_state;
use rigorix_mcp::host::error::HostError;

/// A catalog-method executor.
#[async_trait]
pub trait MethodBackend: Send + Sync {
    /// Execute a catalog method with JSON params.
    ///
    /// # Errors
    /// Returns a taxonomy-typed [`HostError`] for any failure.
    async fn dispatch(&self, method: &str, params: Value) -> Result<Value, HostError>;

    /// Whether an IdP is configured (auth-aware deployments). Default `false`
    /// (local/legacy mode — session methods are not refused).
    async fn auth_configured(&self) -> bool {
        false
    }

    /// Whether the caller has an attested session. Default `false`.
    async fn is_authenticated(&self) -> bool {
        false
    }
}

/// Production backend: delegates to the composed `rigorix-mcp` host.
///
/// Requires `rigorix_mcp::host::init_host` to have been called (the server's
/// `main` does this before binding).
pub struct HostBackend;

#[async_trait]
impl MethodBackend for HostBackend {
    async fn dispatch(&self, method: &str, params: Value) -> Result<Value, HostError> {
        let entry =
            crate::catalog::find(method).ok_or_else(|| HostError::method_not_found(method))?;
        match entry.mcp_tool {
            Some(tool) => app_state().handle_tool_call(tool, &params).await,
            None => Err(HostError::not_enabled(format!(
                "{method} is not enabled in the OSS reference host"
            ))),
        }
    }

    async fn auth_configured(&self) -> bool {
        app_state().auth_handler.is_some()
    }

    async fn is_authenticated(&self) -> bool {
        match &app_state().auth_handler {
            Some(handler) => handler.current_engine_identity().await.is_some(),
            None => false,
        }
    }
}
