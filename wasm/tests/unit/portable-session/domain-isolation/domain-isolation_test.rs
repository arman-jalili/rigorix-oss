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

// ---------------------------------------------------------------------------
// The CI gate and the test must not drift (issue #1311)
// ---------------------------------------------------------------------------

fn gate_script() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/check-domain-isolation.sh"),
    )
    .expect("domain isolation gate exists")
}

#[test]
fn the_ci_gate_bans_every_symbol_the_test_bans() {
    // Two checks that drift apart are worse than one: the script is what CI runs, the test
    // is what a contributor sees locally.
    let script = gate_script();
    for banned in FORBIDDEN {
        assert!(
            script.contains(banned),
            "the CI gate does not ban {banned}, which the test bans"
        );
    }
    for outward in [
        "crate::infrastructure",
        "crate::interfaces",
        "crate::application",
    ] {
        assert!(
            script.contains(outward),
            "the CI gate does not forbid {outward} from domain/"
        );
    }
}

#[test]
fn the_ci_gate_fails_closed_and_skips_comments() {
    let script = gate_script();
    // A gate that passes when it could not look is not a gate.
    assert!(script.contains("the gate cannot look"));
    assert!(script.contains("exit 1"));
    // And documentation may still name the forbidden things — that is how the rule is
    // explained, so comment lines are stripped rather than the docs being made vague.
    assert!(script.contains("strip_comments"));
}

#[test]
fn the_gate_covers_more_capabilities_than_the_rust_test() {
    // The script is the stricter of the two: it also forbids the process environment, the
    // network and an async runtime, which a source scan in Rust would otherwise have to
    // duplicate.
    let script = gate_script();
    for extra in ["std::process", "std::net", "tokio", "OsRng", "getrandom"] {
        assert!(script.contains(extra), "the CI gate misses {extra}");
    }
}
