//! `rigorix.system.version` (#888 OSS-C2).
//!
//! Reports the host name, engine version, native API version, the schema set
//! the host is bound to, and the capabilities it advertises.

use serde_json::{Value, json};

/// The frozen native API version (`catalog.json` `apiVersion`).
pub const API_VERSION: &str = "1.0.0";

/// Build the `rigorix.system.version` result.
pub fn system_version() -> Value {
    json!({
        "name": "rigorix-server",
        "engine_version": rigorix_engine::ENGINE_VERSION,
        "api_version": API_VERSION,
        "schemas": [
            "policy.json",
            "envelope.json",
            "claims.json",
            "api/catalog.json",
            "api/errors.json",
            "api/events.json",
        ],
        "capabilities": [
            "jsonrpc-2.0",
            "batch",
            "mcp-parity",
        ],
    })
}
