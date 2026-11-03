//! Contract: exactly the four catalog services, named as the exploration froze them.
//!
//! Issue: #1294 (contract freeze)

use crate::src_files;

#[test]
fn the_catalog_services_exist_with_their_frozen_names() {
    let files: Vec<String> = src_files().into_iter().map(|(p, _)| p).collect();
    for service in [
        "src/application/open_replay_session.rs",
        "src/application/attempt_run.rs",
        "src/application/export_artifact_set.rs",
        "src/application/run_conformance_vector.rs",
    ] {
        assert!(
            files.iter().any(|f| f.ends_with(service)),
            "missing catalog service: {service}"
        );
    }
}

#[test]
fn verification_is_not_an_application_service() {
    // ADR-013 removed `VerifyExportedArtifacts`: the crate produces, the host verifies.
    // A file named after it would mean the decision was quietly reversed.
    let files: Vec<String> = src_files().into_iter().map(|(p, _)| p).collect();
    assert!(
        !files
            .iter()
            .any(|f| f.contains("verify_exported_artifacts")),
        "verification must not return to the crate"
    );
}

#[test]
fn the_application_layer_names_the_engine_only_through_the_seam() {
    // "Borrowed, never re-modelled": application code may talk to `GateEvaluator`, not to
    // a policy type of its own invention.
    let mut offenders = Vec::new();
    for (path, body) in src_files() {
        if !path.contains("/application/") {
            continue;
        }
        for line in crate::code_lines(&body) {
            if line.contains("rigorix_engine::") {
                offenders.push(format!("{path}: {line}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "engine types belong behind the seam:\n{}",
        offenders.join("\n")
    );
}
