//! Contract: this crate ships at the engine's version (AC 24, 27; ADR-011).
//!
//! Issue: #1294 (contract freeze)

use crate::read_manifest;

fn declared_version(manifest: &str) -> String {
    manifest
        .lines()
        .find(|line| line.trim_start().starts_with("version ="))
        .and_then(|line| line.split('"').nth(1))
        .map(str::to_string)
        .expect("a version in the manifest")
}

#[test]
fn the_crate_version_equals_the_engine_version() {
    // ADR-011: one release train. A host that pins `rigorix-gate-wasm` and
    // `rigorix-engine` must never have to reconcile two numbers.
    let ours = declared_version(&read_manifest("Cargo.toml"));
    let engine = declared_version(&read_manifest("../engine/Cargo.toml"));
    assert_eq!(ours, engine, "crate version must track rigorix-engine");
}

#[test]
fn the_crate_identity_is_the_one_the_adr_names() {
    // The crate name and the directory are a public commitment (ADR-011); renaming is a
    // breaking change for anyone who added the dependency.
    let manifest = read_manifest("Cargo.toml");
    assert!(manifest.contains("name = \"rigorix-gate-wasm\""));
    assert!(manifest.contains("license = \"MIT OR Apache-2.0\""));
}

#[test]
fn the_engine_pin_is_exact_path_and_version() {
    // When the engine dependency lands it must be `path + version`, so a workspace build
    // cannot silently use a different engine than the one published alongside.
    let manifest = read_manifest("Cargo.toml");
    assert!(!manifest.contains("rigorix-engine = { path = \"../engine\" }"));
}
