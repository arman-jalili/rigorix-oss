//! Contract: no verifier, no network stack, no runtime in the bundle (AC 14, 23).
//!
//! Issue: #1294 (contract freeze)

use crate::read_manifest;

/// Dependencies that must never appear in the shipped dependency graph (NFR-002).
const BANNED: [&str; 8] = [
    "rigorix-verifier",
    "tokio",
    "reqwest",
    "tree-sitter",
    "prometheus",
    "tracing-appender",
    "libc",
    "hyper",
];

#[test]
fn the_manifest_declares_no_banned_dependency() {
    let manifest = read_manifest("Cargo.toml");
    let deps = manifest
        .split("[dependencies]")
        .nth(1)
        .unwrap_or_default()
        .split("[lints")
        .next()
        .unwrap_or_default();
    for banned in BANNED {
        assert!(
            !deps.contains(banned),
            "banned dependency in the bundle graph: {banned}"
        );
    }
}

#[test]
fn no_source_file_names_a_verifier() {
    // A comment may explain *why* there is no verifier (ADR-013). Code may not call one.
    for (path, body) in crate::src_files() {
        for line in crate::code_lines(&body) {
            for banned in ["rigorix_verifier", "rigorix-verifier"] {
                assert!(
                    !line.contains(banned),
                    "{path} references a verifier in code: {line}"
                );
            }
        }
    }
}

#[test]
fn the_crate_is_a_library_and_a_cdylib() {
    // cdylib is what a host actually loads; rlib is what the tests link against.
    let manifest = read_manifest("Cargo.toml");
    assert!(manifest.contains("\"rlib\""));
    assert!(manifest.contains("\"cdylib\""));
}
