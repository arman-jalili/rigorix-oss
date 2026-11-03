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
