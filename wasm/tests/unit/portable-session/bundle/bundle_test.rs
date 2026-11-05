//! Contract: the bundle stays inside its budget and its dependency ban list (AC 23).
//!
//! Issue: #1294 (contract freeze)

use crate::read_manifest;

/// The dependency floor: serde, serde_json, thiserror. Everything else needs a reason.
// sha2 is the engine's own hash crate: a different implementation would produce
// different digests for the same bytes, which is the one thing this crate may not do.
/// Each one costs bundle size and needs a decision. The three additions are #1383's:
/// `rigorix-engine` (the gate, behind the opt-in `engine` feature — ADR-014), `async-trait`
/// (the engine's port traits are `async_trait`, so implementing them without it is not
/// possible), `chrono` (the engine records *when* a provider was consulted, and an instant
/// this host invented would be a fabrication inside evidence).
const ALLOWED_DEPENDENCIES: [&str; 8] = [
    "serde",
    "serde_json",
    "sha2",
    "thiserror",
    "rigorix-engine",
    "async-trait",
    "chrono",
    "uuid",
];

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
    // C.6 (#1386) solved the *size*; #1400 narrowed the wasm32 tokio declaration to `sync`,
    // which is an absence of runtime rather than an unused dependency; #1401 replaced the
    // independence gate's crate-name ban with the property it was always trying to state.
    // Those three are the whole reason this line could flip, so the contract now asserts the
    // flip rather than the block: the shipped artifact decides.
    assert!(
        manifest.contains("default = [\"engine\"]"),
        "the shipped default must evaluate: an artifact that cannot decide is not a gate (#1401)"
    );
}

#[test]
fn the_engine_dependency_is_optional_by_contract() {
    let manifest = read_manifest("Cargo.toml");
    // On by default since #1401 — and still *optional*, because `--no-default-features` is the
    // contract-only build: it compiles, reports `gate_evaluation: false`, and refuses at
    // `open_session`. Both configurations ship a truth; one of them just cannot decide.
    assert!(
        manifest.contains("default = [\"engine\"]"),
        "the default build must be the one that evaluates (#1401)"
    );
    // Declared once per target (#1401): host builds link the engine whole, wasm32 links the
    // slice. Both must stay optional, or `--no-default-features` loses its meaning.
    let engine_lines: Vec<&str> = manifest
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("rigorix-engine ="))
        .collect();
    assert_eq!(
        engine_lines.len(),
        2,
        "one declaration per target, host and wasm32: {engine_lines:?}"
    );
    for line in &engine_lines {
        assert!(
            line.contains("optional = true"),
            "the engine must stay optional so --no-default-features still builds: {line}"
        );
    }
}

#[test]
fn the_portable_slice_is_scoped_to_wasm32() {
    // #1401, the regression this test exists for: `portable` is a *subtractive* feature — it
    // `cfg`s native modules out. Declared unconditionally (in `[dependencies]`, or through a
    // `rigorix-engine/portable` entry in the `engine` feature), any workspace build that
    // includes this crate unifies it into `mcp` and `actions`, and `JwksVerifier`,
    // `orchestrator` and `plan_validation` vanish from under them. That is exactly what
    // `cargo clippy --workspace --all-targets` reported once `default = ["engine"]` landed.
    //
    // The property is about *where* the slice is enabled, not whether the string appears.
    let manifest = read_manifest("Cargo.toml");

    let features = manifest
        .split("[features]")
        .nth(1)
        .unwrap_or_default()
        .split("\n[")
        .next()
        .unwrap_or_default();
    assert!(
        !features.contains("rigorix-engine/portable"),
        "the `engine` feature must not enable the slice for every target — it subtracts native \
         modules from any crate that shares the graph"
    );

    let wasm32 = manifest
        .lines()
        .position(|line| line.trim() == "[target.'cfg(target_arch = \"wasm32\")'.dependencies]")
        .expect("the wasm32 target table exists");
    let wasm32_block: String = manifest
        .lines()
        .skip(wasm32 + 1)
        .take_while(|line| !line.starts_with('['))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        wasm32_block.contains("features = [\"portable\"]"),
        "the wasm32 declaration must name the portable slice: {wasm32_block}"
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
    // And the crate's own tests and its wasm release build, which are gates too. The build
    // goes through `ci/wasm-build.sh` because the size profile lives there (#1386) — a gate
    // that called `cargo build` directly would measure a different artifact than the one
    // `bundle-budget.txt` was derived from.
    assert!(runner.contains("cargo test"));
    assert!(
        runner.contains("ci/wasm-build.sh"),
        "the gates must build through the script that owns the release profile"
    );
    gates_runner_contains_build_profile();
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
    // And it is counted anyway (#1421): the summary states the fraction, so "N gates passed"
    // cannot be read as full coverage of a suite whose declared gate did not run.
    assert!(
        runner.contains("of $DECLARED_GATES gate(s) ran"),
        "the summary does not state how many of the declared gates ran"
    );
    assert!(
        !runner.contains("gate(s) passed"),
        "a bare 'N gate(s) passed' understates a suite with a declared gate that did not run"
    );
}

#[test]
fn ci_invokes_the_runner_and_the_gate_treats_it_as_mandatory() {
    let workflow = read_manifest("../.github/workflows/ci.yml");
    assert!(
        workflow.contains("wasm/ci/run-wasm-gates.sh"),
        "CI does not run the wasm gates"
    );
    // #1421: host verification is a *declared* gate of the suite — declared inside the
    // `--with-npm` branch, which is why the count could not see it. CI must ask for it by
    // that flag: a separate `run-host-verification.sh` step would declare the same gate twice
    // and leave the suite's own count at ten.
    assert!(
        workflow.contains("wasm/ci/run-wasm-gates.sh --with-npm"),
        "CI does not run the independence check as part of the declared gate set"
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

/// The build the budget assumes is the build every gate performs.
#[test]
fn gates_runner_contains_build_profile() {
    let build = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/wasm-build.sh"),
    )
    .expect("the build script is in the tree");
    for setting in [
        "CARGO_PROFILE_RELEASE_LTO=fat",
        "CARGO_PROFILE_RELEASE_OPT_LEVEL=z",
        "CARGO_PROFILE_RELEASE_PANIC=abort",
        "CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1",
    ] {
        assert!(
            build.contains(setting),
            "the size profile is missing {setting}; the measured number would not describe \
             the artifact this build produces"
        );
    }
}
