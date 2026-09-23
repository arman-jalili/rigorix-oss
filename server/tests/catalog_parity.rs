//! Catalog drift + MCP parity (#888 OSS-C2, ADR-0001 D9).
//!
//! The server must serve **exactly** the frozen `rigorix.*` catalog, and every
//! MCP tool must map 1:1 to a catalog method. When `RIGORIX_SDK_SCHEMAS` is
//! set (the OSS CI conformance job), the in-code catalog is compared
//! field-for-field against `rigorix-sdk/schemas/api/catalog.json`.

use std::collections::{BTreeMap, BTreeSet};

use rigorix_server::catalog::{AuthLevel, CATALOG, find};

#[test]
fn catalog_has_nineteen_unique_frozen_methods() {
    assert_eq!(CATALOG.len(), 19, "catalog.json v1 freezes 19 methods");
    let names: BTreeSet<&str> = CATALOG.iter().map(|entry| entry.name).collect();
    assert_eq!(names.len(), CATALOG.len(), "method names must be unique");
}

#[test]
fn server_serves_exactly_the_catalog_set() {
    // A known method resolves; an unlisted one does not.
    assert!(find("rigorix.plan").is_some());
    assert!(find("rigorix.system.version").is_some());
    assert!(find("rigorix.executeStep").is_none());
}

#[test]
fn no_raw_step_execution_method_exists() {
    // ADR-0001 D10: the catalog is governed execution only — no raw step
    // execution method may be added.
    for entry in CATALOG {
        let lower = entry.name.to_ascii_lowercase();
        assert!(
            !lower.contains("step") || entry.name == "rigorix.plan",
            "unexpected raw step-execution method: {}",
            entry.name
        );
    }
}

/// MCP tools registered by the stdio host (core descriptors + the separately
/// registered auth descriptors).
fn host_mcp_tools() -> BTreeSet<String> {
    let mut tools: BTreeSet<String> = rigorix_mcp::host::all_tool_descriptors()
        .iter()
        .filter_map(|d| d["name"].as_str().map(str::to_string))
        .collect();
    for descriptor in [
        rigorix_mcp::auth::interfaces::mcp::rigorix_auth_login_tool_descriptor(),
        rigorix_mcp::auth::interfaces::mcp::rigorix_auth_status_tool_descriptor(),
        rigorix_mcp::auth::interfaces::mcp::rigorix_auth_logout_tool_descriptor(),
    ] {
        if let Some(name) = descriptor["name"].as_str() {
            tools.insert(name.to_string());
        }
    }
    tools
}

#[test]
fn every_mcp_tool_maps_one_to_one_to_a_catalog_method() {
    let catalog_tools: BTreeSet<String> = CATALOG
        .iter()
        .filter_map(|entry| entry.mcp_tool.map(str::to_string))
        .collect();
    assert_eq!(
        catalog_tools,
        host_mcp_tools(),
        "MCP tools must map 1:1 to catalog methods (ADR-0001 D9)"
    );
}

#[test]
fn sdk_catalog_matches_when_schemas_are_available() {
    let Ok(dir) = std::env::var("RIGORIX_SDK_SCHEMAS") else {
        eprintln!("RIGORIX_SDK_SCHEMAS unset — skipping SDK catalog drift check");
        return;
    };
    let path = std::path::Path::new(&dir).join("api/catalog.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let sdk: serde_json::Value = serde_json::from_str(&text).expect("catalog.json parses");

    let mut expected: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for method in sdk["methods"].as_array().expect("methods array") {
        let name = method["name"].as_str().expect("method name").to_string();
        let auth = method["auth"].as_str().expect("auth").to_string();
        let mcp_tool = method["mcpTool"].as_str().map(str::to_string);
        expected.insert(name, (auth, mcp_tool));
    }

    let actual: BTreeMap<String, (String, Option<String>)> = CATALOG
        .iter()
        .map(|entry| {
            (
                entry.name.to_string(),
                (
                    entry.auth.as_str().to_string(),
                    entry.mcp_tool.map(str::to_string),
                ),
            )
        })
        .collect();

    assert_eq!(
        actual, expected,
        "in-code catalog must match rigorix-sdk api/catalog.json (name/auth/mcpTool)"
    );
}

#[test]
fn auth_level_tokens_match_catalog() {
    assert_eq!(AuthLevel::Public.as_str(), "public");
    assert_eq!(AuthLevel::Session.as_str(), "session");
    assert_eq!(AuthLevel::Admin.as_str(), "admin");
}
