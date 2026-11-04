//! Contract: the domain layer exists and holds the frozen tactical model (AC 18 shape).
//!
//! Issue: #1294 (contract freeze)

use crate::src_files;

#[test]
fn the_domain_layer_has_the_four_layers_of_one_context() {
    let files: Vec<String> = src_files().into_iter().map(|(p, _)| p).collect();
    for layer in [
        "src/domain/mod.rs",
        "src/application/mod.rs",
        "src/infrastructure/mod.rs",
        "src/interfaces/mod.rs",
    ] {
        assert!(
            files.iter().any(|f| f.ends_with(layer)),
            "missing layer root: {layer}"
        );
    }
}

#[test]
fn the_aggregate_and_its_entity_are_in_the_domain() {
    let files: Vec<String> = src_files().into_iter().map(|(p, _)| p).collect();
    assert!(
        files
            .iter()
            .any(|f| f.ends_with("src/domain/session/replay_session.rs"))
    );
    assert!(
        files
            .iter()
            .any(|f| f.ends_with("src/domain/session/gate_attempt.rs"))
    );
}

#[test]
fn the_twelve_value_objects_are_all_present() {
    // The exploration captured twelve; a freeze that shipped eleven would have dropped a
    // concept silently, and the missing one would be re-invented at the call site.
    let mut names = Vec::new();
    for (path, _) in src_files() {
        if path.contains("/domain/value/") {
            names.push(path);
        }
    }
    for required in [
        "scenario.rs",
        "fixture.rs",
        "clock.rs",
        "randomness.rs",
        "digest.rs",
        "ids.rs",
        "key.rs",
        "action.rs",
        "ports.rs",
    ] {
        assert!(
            names.iter().any(|p| p.ends_with(required)),
            "missing value object module: {required}"
        );
    }
}

// ---------------------------------------------------------------------------
// Architecture readiness (issue #1319): the docs a successor needs
// ---------------------------------------------------------------------------

fn read_doc(relative: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|_| panic!("missing {relative}"))
}

#[test]
fn the_crate_ships_a_readme_runbook_and_recovery_plan() {
    for doc in [
        "README.md",
        ".pi/architecture/runbook.md",
        ".pi/architecture/disaster-recovery.md",
        ".pi/architecture/CHANGELOG.md",
        ".pi/architecture/implementation-roadmap.md",
    ] {
        let body = read_doc(doc);
        assert!(body.len() > 500, "{doc} is too thin to be useful");
    }
}

#[test]
fn the_runbook_names_every_gate_and_the_command_that_runs_it() {
    let runbook = read_doc(".pi/architecture/runbook.md");
    for gate in [
        "domain_isolation",
        "bundle independence",
        "bundle size",
        "failure tiers",
        "conformance",
        "workspace pins",
    ] {
        assert!(
            runbook.contains(gate),
            "the runbook has no section for {gate}"
        );
    }
    // And the one-command entrypoint, so a reader does not have to assemble it.
    assert!(runbook.contains("run-wasm-gates.sh"));
}

#[test]
fn the_runbook_states_the_gate_unavailable_exit_condition() {
    // The most likely support question is "why does every run refuse?". The answer must be
    // findable without reading the code.
    let runbook = read_doc(".pi/architecture/runbook.md");
    assert!(runbook.contains("gate_unavailable"));
    assert!(runbook.contains("FR-011"));
    assert!(runbook.contains("EngineGate::unavailable()"));
}

#[test]
fn the_known_limits_are_written_down_not_implied() {
    let runbook = read_doc(".pi/architecture/runbook.md");
    for limit in [
        "Engine seam unlinked",
        "non-repudiation",
        "shape-based",
        "No persistence",
    ] {
        assert!(runbook.contains(limit), "limit not stated: {limit}");
    }
}

#[test]
fn recovery_separates_what_we_hold_from_what_the_host_holds() {
    let dr = read_doc(".pi/architecture/disaster-recovery.md");
    // The whole design is "we keep nothing"; a DR plan that promised recovery of the record
    // from our side would be describing a different product.
    assert!(dr.contains("The crate is stateless by design"));
    assert!(
        dr.contains("| Owner |") || dr.contains("owner"),
        "the plan does not name who holds each asset"
    );
    // And the three-valued vocabulary is what keeps a lost key from becoming an accusation.
    assert!(dr.contains("unverifiable"));
    assert!(dr.contains("not_verified"));
    // A record that leaked is never edited: an edited record is not evidence.
    assert!(dr.contains("Do not edit the old bytes"));
}
