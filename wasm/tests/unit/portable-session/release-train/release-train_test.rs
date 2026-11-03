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
    // cannot silently use a different engine than the one published alongside. It is absent
    // today (FR-011), and absent beats unpinned.
    let manifest = read_manifest("Cargo.toml");
    assert!(!manifest.contains("rigorix-engine = { path = \"../engine\" }"));
}

#[test]
fn the_crate_is_a_workspace_member() {
    // ADR-011/criterion 27: a crate outside `[workspace] members` is a crate nobody
    // versions, tests or publishes on the train.
    let workspace = read_manifest("../Cargo.toml");
    let members = workspace
        .split("members = [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("members list");
    assert!(
        members.contains("\"wasm\""),
        "the crate is not registered in the workspace: {members}"
    );
    // And it must not declare itself as its own workspace root any more: two roots make two
    // lockfiles and two ideas of the dependency graph.
    assert!(!read_manifest("Cargo.toml").contains("[workspace]"));
}

#[test]
fn the_release_gate_names_the_crate_in_its_version_loop() {
    // An excluded crate is an unreleased crate. The release workflow fails the build when a
    // crate's version does not equal the tag; if the crate is not in the loop, nothing checks
    // it and it ships stale — which is exactly what happened to `actions` for seven weeks.
    let release = read_manifest("../.github/workflows/release.yml");
    let loop_line = release
        .lines()
        .find(|line| line.contains("for crate in"))
        .expect("the version-consistency loop");
    assert!(
        loop_line.contains("wasm"),
        "the release gate does not version-check this crate: {loop_line}"
    );
}
