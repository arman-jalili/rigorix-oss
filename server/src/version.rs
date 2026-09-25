//! `rigorix.system.version` (#888 OSS-C2).
//!
//! Reports the host name, engine version, native API version, the schema set
//! the host is bound to, the capabilities it advertises, and — ADR-016 Phase C
//! (#899) — the active `history_integrity` mode plus the anchor identity/head.

use serde_json::{Value, json};

/// The frozen native API version (`catalog.json` `apiVersion`).
pub const API_VERSION: &str = "1.0.0";

/// Build the `rigorix.system.version` result.
pub fn system_version() -> Value {
    let anchor = crate::anchor::status();
    json!({
        "name": "rigorix-server",
        "engine_version": rigorix_engine::ENGINE_VERSION,
        "api_version": API_VERSION,
        "history_integrity": crate::anchor::mode_label(anchor.mode),
        "anchor": {
            "id": anchor.anchor_id,
            "head": anchor.head,
        },
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
