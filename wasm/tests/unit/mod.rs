//! Contract tests for `rigorix-gate-wasm` (contract freeze, #1294).
//!
//! @canonical .pi/architecture/modules/portable-session.md
//! Issue: #1294 (contract freeze)
//!
//! One module per frozen component. These tests assert the **shape** of the contracts —
//! signatures, enum cardinality, DTO strictness, layer isolation, version parity — not
//! behaviour. Behaviour arrives with each implementation issue and its own tests.
//!
//! A test in this tree that cannot compile is the intended guard for several criteria:
//! adding a `RefusalKind` without wording, a domain error without a projection, or a
//! fifth exported function all break the build rather than sliding through review.

/// Test doubles for the ports and the engine gate (issue #1296).
mod support;

#[cfg(feature = "engine")]
#[path = "portable-session/enginegate/enginegate_test.rs"]
mod enginegate;

#[path = "portable-session/replaysession/replaysession_test.rs"]
mod replaysession;

#[path = "portable-session/replaysession-open/replaysession-open_test.rs"]
mod replaysession_open;

#[path = "portable-session/replaysession-attempt/replaysession-attempt_test.rs"]
mod replaysession_attempt;

#[path = "portable-session/replaysession-seal/replaysession-seal_test.rs"]
mod replaysession_seal;

#[path = "portable-session/refusalvocabulary/refusalvocabulary_test.rs"]
mod refusalvocabulary;

#[path = "portable-session/evidenceprojection/evidenceprojection_test.rs"]
mod evidenceprojection;

#[path = "portable-session/attemptrun/attemptrun_test.rs"]
mod attemptrun;

#[path = "portable-session/exportartifactset/exportartifactset_test.rs"]
mod exportartifactset;

#[path = "portable-session/export-stamp/export-stamp_test.rs"]
mod export_stamp;

#[path = "portable-session/host-verification/host-verification_test.rs"]
mod host_verification;

#[path = "portable-session/bundle-independence/bundle-independence_test.rs"]
mod bundle_independence;

#[path = "portable-session/wasmcontract/wasmcontract_test.rs"]
mod wasmcontract;

#[path = "portable-session/telemetryobserver/telemetryobserver_test.rs"]
mod telemetryobserver;

// The crate's public surface, named by a test (#1427). The interface floor caught fourteen
// new uncovered lines in `wasm/src/interfaces/` that no unit test executed; this module is
// the subject that was missing.
#[path = "interfaces/interfaces_test.rs"]
mod interfaces;

#[path = "portable-session/domain-isolation/domain-isolation_test.rs"]
mod domain_isolation;

#[path = "portable-session/refusaldetail/refusaldetail_test.rs"]
mod refusaldetail;

#[cfg(feature = "conformance")]
#[path = "portable-session/runconformancevector/runconformancevector_test.rs"]
mod runconformancevector;

#[cfg(feature = "conformance")]
#[path = "portable-session/seeded-divergence/seeded-divergence_test.rs"]
mod seeded_divergence;

#[path = "portable-session/bundle/bundle_test.rs"]
mod bundle;

#[path = "portable-session/release-train/release-train_test.rs"]
mod release_train;

// The cross-process determinism subject is a vector replay: without the `conformance`
// feature there is no harness to replay with, so the module is gated with it (#1385).
#[cfg(feature = "conformance")]
#[path = "portable-session/determinism/determinism_test.rs"]
mod determinism;

#[path = "portable-session/failure-tiers/failure-tiers_test.rs"]
mod failure_tiers;

#[path = "portable-session/domain-layer-domain/domain-layer-domain_test.rs"]
mod domain_layer;

#[path = "portable-session/application-layer-application/application-layer-application_test.rs"]
mod application_layer;

#[path = "portable-session/infrastructure-layer-infrastructure/infrastructure-layer-infrastructure_test.rs"]
mod infrastructure_layer;

/// Files under `src/`, as (relative path, contents), discovered at test time.
///
/// Every source-scanning test uses this, so a new layer file is covered without editing
/// a list.
pub(crate) fn src_files() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let name = path
                    .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                let body = std::fs::read_to_string(&path).unwrap_or_default();
                out.push((name, body));
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&root, &mut out);
    out.sort();
    out
}

/// Source lines with comments stripped.
///
/// Doc comments legitimately *name* the forbidden things ("no `std::time` here"); the
/// isolation rule is about code, not prose. Stripping comments keeps the rule precise
/// instead of forcing the docs to be vague.
pub(crate) fn code_lines(src: &str) -> impl Iterator<Item = &str> {
    src.lines().filter(|line| {
        let trimmed = line.trim_start();
        !(trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*'))
    })
}

/// Read a file next to the crate (e.g. `Cargo.toml`, `../engine/Cargo.toml`).
pub(crate) fn read_manifest(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"))
}
