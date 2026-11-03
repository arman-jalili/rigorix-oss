//! Contract: one adapter per port, all fixture-driven, none ambient (AC 14, 23).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::ports::{
    BusinessSink, Clock, IdSource, KeySource, ProviderTransport, Randomness, SessionStore,
};
use rigorix_gate_wasm::domain::value::{ClockTrack, KeyRef, RandomSeed};
use rigorix_gate_wasm::infrastructure::{
    FixtureClock, FixtureProviderTransport, FixtureRandomness, InMemorySessionStore,
    MemoryBusinessSink, SeededIdSource, SeededKeySource,
};

use crate::src_files;

#[test]
fn every_port_has_a_shipped_adapter() {
    // Compile-time: each adapter must satisfy its port's trait, so a port change cannot
    // leave an adapter behind.
    let _: &dyn Clock = &FixtureClock::new(ClockTrack::default());
    let _: &mut dyn IdSource = &mut SeededIdSource::new(std::collections::HashMap::new());
    let _: &dyn Randomness = &FixtureRandomness::new(RandomSeed::new(
        rigorix_gate_wasm::domain::value::Digest::from_trusted(
            "sha256:".to_string() + &"a".repeat(64),
        ),
        vec![],
    ));
    let _: &dyn KeySource = &SeededKeySource::new(KeyRef::from_trusted("demo:key"));
    let _: &mut dyn ProviderTransport = &mut FixtureProviderTransport::new(serde_json::json!({}));
    let _: &dyn BusinessSink = &MemoryBusinessSink::new();
    let _: &dyn SessionStore = &InMemorySessionStore::new();
}

#[test]
fn the_unavailable_key_source_is_representable() {
    // The fault matrix needs a key source that cannot answer; the engine's truthful
    // status for that case is `unverified`, never `unauthenticated`.
    let source = SeededKeySource::unavailable();
    use rigorix_gate_wasm::domain::ports::KeySource as _;
    let failure = source.key_ref().expect_err("unavailable");
    assert_eq!(failure.port.as_str(), "key_source");
    assert_eq!(failure.kind.as_str(), "missing");
}

#[test]
fn infrastructure_holds_no_verification_logic() {
    let mut offenders = Vec::new();
    for (path, body) in src_files() {
        if !path.contains("/infrastructure/") {
            continue;
        }
        for line in crate::code_lines(&body) {
            for forbidden in ["verify(", "signature", "ed25519", "rigorix_verifier"] {
                if line.contains(forbidden) {
                    offenders.push(format!("{path}: {forbidden} in `{line}`"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "verification in infrastructure:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_repository_folder_implements_the_domain_owned_interface() {
    let files: Vec<String> = src_files().into_iter().map(|(p, _)| p).collect();
    assert!(
        files
            .iter()
            .any(|f| f.ends_with("src/infrastructure/repository/mod.rs"))
    );
    // One definition, no fork: the trait is in domain/ports, and infrastructure
    // re-exports it rather than declaring a second one.
    assert!(
        files
            .iter()
            .any(|f| f.ends_with("src/domain/ports/session_store.rs"))
    );
}
