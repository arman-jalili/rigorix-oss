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
