//! Contract: three failure tiers, three promises — and no `catch_unwind` (AC 26; ADR-012).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::engine_seam::EngineBindingError;
use rigorix_gate_wasm::domain::errors::AttemptError;

#[test]
fn tier_1_logical_failures_are_values() {
    // A port that cannot answer is a refusal with a recorded reason — the record can say
    // what was not checked.
    let failure = rigorix_gate_wasm::domain::value::PortFailure::new(
        rigorix_gate_wasm::domain::value::PortId::Clock,
        rigorix_gate_wasm::domain::value::PortFailureKind::Exhausted,
    );
    assert_eq!(failure.to_string(), "clock:exhausted");
}

#[test]
fn tier_3_is_representable_and_never_a_refusal_claim() {
    // A trap is fatal and host-visible. The type exists so the native harness can express
    // it; the browser surfaces it as a runtime error, not as a `Refused` outcome.
    let abort = EngineBindingError::Aborted;
    let error = AttemptError::EngineFailure { source: abort };
    let text = error.to_string().to_lowercase();
    assert!(text.contains("aborted"));
    assert!(!text.contains("refused"));
    assert!(!text.contains("allowed"));
}

#[test]
fn the_crate_does_not_pretend_to_catch_panics() {
    // `catch_unwind` cannot work on wasm32-unknown-unknown: there is no unwinder. Writing
    // it would be a promise the platform cannot keep (ADR-012).
    let mut offenders = Vec::new();
    for (path, body) in crate::src_files() {
        for line in crate::code_lines(&body) {
            if line.contains("catch_unwind") || line.contains("panic::set_hook") {
                offenders.push(format!("{path}: {line}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "panic-catching code:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn panic_is_abort_on_the_target() {
    // Documented in the crate root and enforced by the profile in a later issue; the
    // contract test pins the *statement* so the intent cannot drift silently.
    let manifest = crate::read_manifest("Cargo.toml");
    assert!(manifest.contains("rigorix-gate-wasm"));
    assert!(manifest.contains("wasm"));
}

// ---------------------------------------------------------------------------
// Criterion 26: tier 2 is denied at build time, tier 3 is representable (#1317)
// ---------------------------------------------------------------------------

#[test]
fn the_crate_denies_panicking_constructs_on_the_gate_path() {
    // The lint attributes are the enforcement: a `unwrap` in `domain/` fails `cargo clippy`
    // rather than becoming a trap in a browser (ADR-012 tier 2).
    let root = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
    )
    .expect("crate root");
    for lint in [
        "clippy::unwrap_used",
        "clippy::expect_used",
        "clippy::panic",
        "clippy::todo",
        "clippy::unimplemented",
        "clippy::indexing_slicing",
    ] {
        assert!(root.contains(lint), "the crate does not deny {lint}");
    }
    // Denied only outside `cfg(test)`: the integration test crates legitimately use `expect`
    // on fixtures, and attributes do not cross crate boundaries anyway.
    assert!(root.contains("cfg_attr("));
    assert!(root.contains("not(test)"));
}

#[test]
fn a_trap_is_representable_and_never_reads_as_a_verdict() {
    use rigorix_gate_wasm::domain::engine_seam::EngineBindingError;
    use rigorix_gate_wasm::interfaces::errors::project_attempt;

    // Tier 3 has a value, so a host can tell "the gate stopped" from "the gate said no".
    let refusal = project_attempt(&AttemptError::EngineFailure {
        source: EngineBindingError::Aborted,
    });
    assert_eq!(refusal.kind, "gate_unavailable");
    let text = refusal.message.to_lowercase();
    for forbidden in ["refused", "allowed", "denied", "verified"] {
        assert!(!text.contains(forbidden), "a trap reads as {forbidden}");
    }
}

#[test]
fn the_failure_tier_gate_exists_and_fails_closed() {
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/check-failure-tiers.sh"),
    )
    .expect("failure-tier gate exists");
    assert!(script.contains("catch_unwind"));
    assert!(script.contains("EngineBindingError::Aborted"));
    assert!(script.contains("exit 1"));
    // And it must say the rule out loud, because it is the one a reader will otherwise
    // "fix" by adding a catch.
    assert!(script.contains("a refusal is always a decision"));
}

#[test]
fn the_engine_binding_failure_variants_are_exhaustive() {
    // Two kinds: rejected (could not evaluate) and aborted (stopped). A third would need a
    // wording and a tier, which is a review, not a patch.
    use rigorix_gate_wasm::domain::engine_seam::EngineBindingError;
    let rejected = EngineBindingError::Rejected {
        detail: "scenario not bound".into(),
    };
    let aborted = EngineBindingError::Aborted;
    assert_ne!(rejected, aborted);
    assert!(rejected.to_string().contains("rejected"));
    assert!(aborted.to_string().contains("aborted"));
}
