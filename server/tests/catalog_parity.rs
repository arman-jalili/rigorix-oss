//! Catalog drift + MCP parity (#888 OSS-C2, ADR-0001 D9).
//!
//! The server serves a **SUBSET** of the single closed `rigorix.*` catalog
//! (ADR-0001 D3: "a server implements a subset and MUST NOT extend the
//! namespace"). When `RIGORIX_SDK_SCHEMAS` is set (the OSS CI conformance job),
//! every in-code method must be *declared* in `rigorix-sdk/schemas/api/catalog.json`
//! and its `auth` / `mcpTool` fields must match — never a set/count equality.
//!
//! The canonical guard is `rigorix-verifier::verify_catalog_subset` (SDK) — a
//! **published** crates.io crate consumed as a dev-dependency (NOT path/git), so
//! there is ONE implementation of the closed-namespace relation shared by the
//! OSS, SDK, and enterprise guards. The SDK catalog is deliberately larger (it
//! also declares the enterprise-side `rigorix.auth.verify`, which OSS does not
//! implement).

use std::collections::{BTreeMap, BTreeSet};

use rigorix_server::catalog::{AuthLevel, CATALOG, find};

#[test]
fn catalog_has_nineteen_unique_frozen_methods() {
    assert_eq!(CATALOG.len(), 19, "the OSS server implements 19 methods");
    let names: BTreeSet<&str> = CATALOG.iter().map(|entry| entry.name).collect();
    assert_eq!(names.len(), CATALOG.len(), "method names must be unique");
}

#[test]
fn server_serves_a_subset_of_the_catalog() {
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

/// AC1: the subset relation FAILS on any method absent from the SDK catalog
/// (demonstrated with a synthetic OSS method the SDK does not declare).
#[test]
fn subset_check_detects_an_undeclared_method() {
    // Declared methods pass...
    assert!(rigorix_verifier::verify_catalog_subset(&["rigorix.plan", "rigorix.run"]).is_ok());
    // ...an OSS method absent from the frozen catalog is a subset violation.
    let err = rigorix_verifier::verify_catalog_subset(&["rigorix.plan", "rigorix.auth.bogus"])
        .expect_err("an undeclared method must be rejected");
    assert!(
        matches!(&err, rigorix_verifier::VerifyError::UndeclaredMethod(m) if m == "rigorix.auth.bogus"),
        "unexpected: {err:?}"
    );
}

/// ADR-0001 D3 conformance (runs in the CI conformance job with
/// `RIGORIX_SDK_SCHEMAS`): every OSS method is declared in the SDK catalog, with
/// matching `auth` / `mcpTool`. No equality/count coupling — the SDK catalog is
/// larger by design and OSS must not implement `rigorix.auth.verify`.
#[test]
fn oss_catalog_is_a_subset_of_the_sdk_catalog() {
    // The shared guard (ADR-0001 D3): every OSS method must be DECLARED in the
    // frozen `rigorix.*` catalog. `rigorix-verifier` resolves the corpus from
    // `RIGORIX_SDK_SCHEMAS` (CI conformance job) or the embedded
    // `rigorix-schemas` copy — never from the repo layout.
    let oss_names: Vec<&str> = CATALOG.iter().map(|entry| entry.name).collect();
    rigorix_verifier::verify_catalog_subset(&oss_names).unwrap_or_else(|e| {
        panic!(
            "OSS catalog declares method(s) absent from the frozen rigorix.* catalog \
             (ADR-0001 D3 — a server implements a SUBSET and MUST NOT extend the \
             namespace): {e}"
        )
    });

    // Field drift (name/auth/mcpTool) for the methods OSS DOES implement
    // requires the SDK checkout — the CI conformance job sets
    // RIGORIX_SDK_SCHEMAS. Without it, the shared subset guard above has still
    // run against the embedded corpus.
    let Ok(dir) = std::env::var("RIGORIX_SDK_SCHEMAS") else {
        eprintln!(
            "RIGORIX_SDK_SCHEMAS unset — subset guard ran against the embedded corpus; \
             skipping field-drift check"
        );
        return;
    };
    let path = std::path::Path::new(&dir).join("api/catalog.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let sdk: serde_json::Value = serde_json::from_str(&text).expect("catalog.json parses");

    let mut sdk_methods: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for method in sdk["methods"].as_array().expect("methods array") {
        let name = method["name"].as_str().expect("method name").to_string();
        let auth = method["auth"].as_str().expect("auth").to_string();
        let mcp_tool = method["mcpTool"].as_str().map(str::to_string);
        sdk_methods.insert(name, (auth, mcp_tool));
    }

    // Field drift for the methods OSS does implement (name/auth/mcpTool).
    for entry in CATALOG {
        let (auth, mcp_tool) = sdk_methods
            .get(entry.name)
            .expect("declared by the shared subset guard above");
        assert_eq!(
            entry.auth.as_str(),
            auth.as_str(),
            "auth drift for '{}'",
            entry.name
        );
        assert_eq!(
            entry.mcp_tool.map(str::to_string),
            *mcp_tool,
            "mcpTool drift for '{}'",
            entry.name
        );
    }

    // No count coupling: the SDK catalog is larger (it declares the
    // enterprise-side `rigorix.auth.verify`, which OSS deliberately omits).
    assert!(
        sdk_methods.len() >= CATALOG.len(),
        "the SDK catalog must be a superset of the OSS subset"
    );
    assert!(
        !oss_names.contains(&"rigorix.auth.verify"),
        "OSS must not implement the enterprise-side rigorix.auth.verify"
    );
}

#[test]
fn auth_level_tokens_match_catalog() {
    assert_eq!(AuthLevel::Public.as_str(), "public");
    assert_eq!(AuthLevel::Session.as_str(), "session");
    assert_eq!(AuthLevel::Admin.as_str(), "admin");
}
