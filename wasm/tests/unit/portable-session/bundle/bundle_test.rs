//! Contract: the bundle stays inside its budget and its dependency ban list (AC 23).
//!
//! Issue: #1294 (contract freeze)

use crate::read_manifest;

/// The dependency floor: serde, serde_json, thiserror. Everything else needs a reason.
const ALLOWED_DEPENDENCIES: [&str; 3] = ["serde", "serde_json", "thiserror"];

#[test]
fn the_dependency_set_is_deliberate() {
    let manifest = read_manifest("Cargo.toml");
    let deps = manifest
        .split("[dependencies]")
        .nth(1)
        .unwrap_or_default()
        .split("[lints")
        .next()
        .unwrap_or_default();

    let declared: Vec<&str> = deps
        .lines()
        .filter_map(|line| line.split('=').next())
        .map(str::trim)
        .filter(|k| !k.is_empty() && !k.starts_with('#'))
        .collect();

    for dep in &declared {
        assert!(
            ALLOWED_DEPENDENCIES.contains(dep),
            "unexpected dependency `{dep}`: each one costs bundle size and needs an ADR"
        );
    }
}

#[test]
fn the_size_gate_is_declared_where_ci_can_read_it() {
    // The budget itself is set from the first green build +25% (NFR-002). Until then the
    // contract is: the crate has no feature that would bloat the bundle.
    let manifest = read_manifest("Cargo.toml");
    assert!(manifest.contains("[features]"));
    // The engine seam is opt-in and off by default: a default-on engine dependency would
    // pull the native stack into the browser bundle.
    assert!(manifest.contains("default = []"));
}

#[test]
fn the_engine_dependency_is_optional_by_contract() {
    let manifest = read_manifest("Cargo.toml");
    // FR-011 has not landed: the seam is declared as a feature with no dependency behind
    // it yet. When it lands, it must be optional and non-default (NFR-005).
    assert!(manifest.contains("engine = []"));
    let deps = manifest.split("[dependencies]").nth(1).unwrap_or_default();
    assert!(
        !deps.contains("rigorix-engine"),
        "the engine must not be a default dependency: the four published crates build from the same commit"
    );
}
