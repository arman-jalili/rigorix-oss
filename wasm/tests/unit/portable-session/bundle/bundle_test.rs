//! Contract: the bundle stays inside its budget and its dependency ban list (AC 23).
//!
//! Issue: #1294 (contract freeze)

use crate::read_manifest;

/// The dependency floor: serde, serde_json, thiserror. Everything else needs a reason.
// sha2 is the engine's own hash crate: a different implementation would produce
// different digests for the same bytes, which is the one thing this crate may not do.
const ALLOWED_DEPENDENCIES: [&str; 4] = ["serde", "serde_json", "sha2", "thiserror"];

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

// ---------------------------------------------------------------------------
// The size gate (issue #1314)
// ---------------------------------------------------------------------------

fn size_gate() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/check-bundle-size.sh"),
    )
    .expect("size gate exists")
}

fn budget_file() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/bundle-budget.txt"),
    )
    .expect("budget file exists")
}

#[test]
fn the_gate_enforces_measured_plus_twenty_five_percent() {
    let script = size_gate();
    // NFR-002: the number is measured first, never guessed, and the ceiling is +25%.
    assert!(script.contains("budget_bytes"));
    assert!(script.contains("measured_bytes"));
    assert!(script.contains("measured + 25%"));
    assert!(script.contains("exit 1"));
}

#[test]
fn the_gate_refuses_to_pass_on_an_unmeasurable_artifact() {
    let script = size_gate();
    // A missing artifact, an unparseable budget and a shipped artifact with no budget are
    // all errors — the gate must not read "nothing to measure" as "within budget".
    assert!(script.contains("a size gate without a budget is an opinion"));
    assert!(script.contains("the budget file is unparseable"));
    assert!(script.contains("a shipped artifact exists but no budget was set"));
}

#[test]
fn the_budget_file_states_which_artifact_it_measures() {
    let budget = budget_file();
    // A budget without a named artifact can be satisfied by measuring the wrong file.
    assert!(budget.contains("artifact: pkg/rigorix_gate_wasm_bg.wasm"));
    assert!(budget.contains("budget_bytes:"));
    // #1349 — the ceiling is in force now that an artifact a host can load exists.
    // Before the export layer the module was 368 B with no exported symbol, so
    // there was no subject to measure; the file said `awaiting-packaging` and the
    // gate said so out loud. Either state must be declared, never implied.
    // Three states, one meaning each: `awaiting-packaging` (no artifact), `packaged`
    // (an artifact exists and the ceiling is in force, nothing published), `shipped`
    // (published — the release-train guard then requires the npm job to gate the
    // release). An undeclared or invented state is a failure here, not a default.
    let state = budget
        .lines()
        .find_map(|l| l.strip_prefix("state:"))
        .map(|v| v.trim().to_string())
        .unwrap_or_default();
    assert!(
        matches!(
            state.as_str(),
            "awaiting-packaging" | "packaged" | "shipped"
        ),
        "the budget file must declare a known state, got {state:?}"
    );
    if state != "awaiting-packaging" {
        // A ceiling with no numbers is a ceiling with no subject.
        let measured: u64 = budget
            .lines()
            .find_map(|l| l.strip_prefix("measured_bytes:"))
            .and_then(|v| v.trim().parse().ok())
            .expect("measured_bytes parses");
        let limit: u64 = budget
            .lines()
            .find_map(|l| l.strip_prefix("budget_bytes:"))
            .and_then(|v| v.trim().parse().ok())
            .expect("budget_bytes parses");
        assert!(
            measured > 0,
            "an artifact that exists has a measurable size"
        );
        // measured + 25% (NFR-002), allowing for integer truncation.
        assert!(
            limit >= measured + measured / 4,
            "budget {limit} must be at least measured + 25% of {measured}"
        );
    }
    assert!(budget.contains("25%"));
}

#[test]
fn the_two_modes_are_explicit_rather_than_implicit() {
    let script = size_gate();
    assert!(script.contains("== shipped artifact"));
    assert!(script.contains("== no shipped artifact yet"));
    // The trend path names why it is not enforcing, instead of exiting quietly.
    assert!(script.contains("ceiling not yet in force"));
}

// ---------------------------------------------------------------------------
// The single gate entrypoint (issue #1318)
// ---------------------------------------------------------------------------

fn gates_runner() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/run-wasm-gates.sh"),
    )
    .expect("gate runner exists")
}

#[test]
fn one_command_runs_every_gate_this_crate_claims() {
    let runner = gates_runner();
    for gate in [
        "check-domain-isolation.sh",
        "check-bundle-independence.sh",
        "check-bundle-size.sh",
        "check-failure-tiers.sh",
        "run-conformance.sh",
        "validate-workspace-pins.sh",
    ] {
        assert!(runner.contains(gate), "the runner does not call {gate}");
    }
    // And the crate's own tests and its wasm release build, which are gates too.
    assert!(runner.contains("cargo test"));
    assert!(runner.contains("--target wasm32-unknown-unknown"));
}

#[test]
fn the_runner_fails_closed_and_labels_what_it_could_not_run() {
    let runner = gates_runner();
    // A failed gate must produce a non-zero exit.
    assert!(runner.contains("FAILED"));
    assert!(runner.contains("exit 1"));
    // Host verification needs npm: it is *labelled* as not run rather than passing silently.
    assert!(runner.contains("Not run (labelled, not silently passing)"));
    assert!(runner.contains("host verification"));
}

#[test]
fn ci_invokes_the_runner_and_the_gate_treats_it_as_mandatory() {
    let workflow = read_manifest("../.github/workflows/ci.yml");
    assert!(
        workflow.contains("wasm/ci/run-wasm-gates.sh"),
        "CI does not run the wasm gates"
    );
    assert!(
        workflow.contains("wasm/ci/run-host-verification.sh"),
        "CI does not run the independence check"
    );

    // A job that runs but does not gate is a report, not a gate.
    let stages = read_manifest("../.github/scripts/ci_stages.sh");
    let mandatory = stages
        .lines()
        .find(|line| line.starts_with("CI_MANDATORY_STAGES="))
        .expect("the stage table");
    assert!(
        mandatory.contains("wasm-gates"),
        "the wasm gates are not mandatory: {mandatory}"
    );
}
