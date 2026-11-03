//! Architecture: `domain/` reaches no ambient capability (AC 18).
//!
//! Issue: #1294 (contract freeze)

use crate::{code_lines, src_files};

/// Symbols whose presence in `domain/` code would mean an ambient capability.
const FORBIDDEN: [&str; 9] = [
    "std::time",
    "std::env",
    "std::fs",
    "std::net",
    "Uuid::new_v4",
    "uuid::Uuid",
    "thread_rng",
    "reqwest",
    "SystemTime",
];

#[test]
fn domain_code_names_no_ambient_capability() {
    let mut violations = Vec::new();
    for (path, body) in src_files() {
        if !path.contains("/domain/") {
            continue;
        }
        for line in code_lines(&body) {
            for forbidden in FORBIDDEN {
                if line.contains(forbidden) {
                    violations.push(format!("{path}: {forbidden} in `{line}`"));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "domain isolation:\n{}",
        violations.join("\n")
    );
}

#[test]
fn domain_does_not_reach_outward() {
    // The dependency rule is one-way: domain may not name the layers that depend on it.
    let mut violations = Vec::new();
    for (path, body) in src_files() {
        if !path.contains("/domain/") {
            continue;
        }
        for line in code_lines(&body) {
            for outward in [
                "crate::infrastructure",
                "crate::interfaces",
                "crate::application",
            ] {
                if line.contains(outward) {
                    violations.push(format!("{path}: {outward} in `{line}`"));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "layer direction:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_domain_owns_its_ports_and_repository() {
    // ADR-002/ADR-008: the traits live in `domain/`, so the aggregate — not an adapter —
    // defines what reconstitution means.
    let files: Vec<String> = src_files().into_iter().map(|(p, _)| p).collect();
    for required in [
        "src/domain/ports/clock.rs",
        "src/domain/ports/id_source.rs",
        "src/domain/ports/randomness.rs",
        "src/domain/ports/key_source.rs",
        "src/domain/ports/provider_transport.rs",
        "src/domain/ports/business_sink.rs",
        "src/domain/ports/session_store.rs",
    ] {
        assert!(
            files.iter().any(|f| f.ends_with(required)),
            "missing port contract: {required}"
        );
    }
}
