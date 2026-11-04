//! Contract: no verifier, no network stack, no runtime in the bundle (AC 14, 23).
//!
//! Issue: #1294 (contract freeze)

use crate::read_manifest;

/// Dependencies that must never appear in the shipped dependency graph (NFR-002).
///
/// `tokio` is not here (#1400, #1401): the slice needs `tokio::sync` for the engine modules that
/// stay, and the prohibition is an async *runtime*, not the crate name. Banning the name refused
/// the design. The property — the wasm32 tokio feature set is `sync` alone — is asserted below and
/// in the gate script, which is what CI runs.
const BANNED: [&str; 7] = [
    "rigorix-verifier",
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

// ---------------------------------------------------------------------------
// The gate script and the tests must not drift (issue #1308)
// ---------------------------------------------------------------------------

#[test]
fn the_bundle_gate_bans_everything_the_tests_ban() {
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/check-bundle-independence.sh"),
    )
    .expect("bundle gate exists");

    // Two checks that drift apart are worse than one: the script is what CI runs, the test
    // is what a contributor sees locally. They must name the same crates.
    for banned in BANNED {
        assert!(
            script.contains(banned),
            "the CI gate does not ban {banned}, which the test bans"
        );
    }
}

#[test]
fn the_bundle_gate_asserts_the_tokio_property_not_the_crate_name() {
    // #1401: `sync` is in the shipped graph, and it has to be. Twice now a manifest line decided
    // a verdict, so the rule is stated as a property the gate can evaluate — "the wasm32 tokio
    // feature set is `sync` alone" — rather than as a crate name that the design legitimately
    // violates.
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/check-bundle-independence.sh"),
    )
    .expect("bundle gate exists");

    assert!(
        script.contains("TOKIO_ALLOWED_FEATURES"),
        "the gate must name the allowed feature set"
    );
    assert!(
        script.contains("tokio feature"),
        "the gate must read tokio's feature edges, not its name"
    );
    assert!(
        !script.contains("\"tokio|"),
        "the gate bans tokio by crate name again — sync is allowed (#1401)"
    );
    // The source half: the defect `check-domain-isolation.sh` could not see, because it looks
    // for `std::time` and a timer can arrive as `tokio::time`.
    assert!(
        script.contains("tokio::(time|spawn|runtime)"),
        "the gate must refuse a timer or a spawn named in source"
    );
}

#[test]
fn the_bundle_gate_fails_closed_when_it_cannot_look() {
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/check-bundle-independence.sh"),
    )
    .expect("bundle gate exists");
    // A gate that passes when the graph could not be computed is the failure mode this
    // project exists to prevent.
    assert!(script.contains("could not compute the dependency tree"));
    assert!(script.contains("exit 1"));
    // And a missing wasm target is a named warning with the fix, not a silent pass.
    assert!(script.contains("rustup target add $TARGET"));
}

#[test]
fn the_crate_is_checked_for_the_wasm_target_itself() {
    // The dependency graph is only meaningful for the target that ships.
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/check-bundle-independence.sh"),
    )
    .expect("bundle gate exists");
    assert!(script.contains("cargo tree --target"));
    assert!(script.contains("cargo check --target"));
}
