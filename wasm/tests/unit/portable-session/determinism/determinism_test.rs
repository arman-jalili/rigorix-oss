//! Contract: identity and bytes are derived, never drawn from the environment (AC 25).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::value::{
    Digest, Draw, RandomSeed, ReplaySessionId, ScenarioFormatVersion, ScenarioRef,
};

fn digest(byte: char) -> Digest {
    Digest::from_trusted(format!("sha256:{}", byte.to_string().repeat(64)))
}

#[test]
fn the_same_inputs_derive_the_same_session_identity() {
    // Criterion 2 in miniature: identity is a digest over the replay key, so two runs of
    // one session cannot disagree about who they are.
    let a = ReplaySessionId::from_digest(digest('a'));
    let b = ReplaySessionId::from_digest(digest('a'));
    assert_eq!(a, b);
    assert_eq!(a.as_str(), b.as_str());
}

#[test]
fn a_different_seed_derives_a_different_identity() {
    // If the seed were left out of the id, two runs with different entropy would claim
    // to be the same session — and the record would be unreproducible.
    let a = ReplaySessionId::from_digest(digest('a'));
    let b = ReplaySessionId::from_digest(digest('b'));
    assert_ne!(a, b);
}

#[test]
fn value_objects_are_equal_by_state_not_by_construction_site() {
    let left = ScenarioRef::new(digest('c'), ScenarioFormatVersion(1));
    let right = ScenarioRef::new(digest('c'), ScenarioFormatVersion(1));
    assert_eq!(left, right);

    // The format version is part of the value: same digest, different declared format is
    // a different scenario reference, not a coincidence.
    let other_version = ScenarioRef::new(digest('c'), ScenarioFormatVersion(2));
    assert_ne!(left, other_version);

    let seed_left = RandomSeed::new(digest('d'), vec![Draw::from_trusted("00")]);
    let seed_right = RandomSeed::new(digest('d'), vec![Draw::from_trusted("00")]);
    assert_eq!(seed_left, seed_right);

    let seed_more_draws = RandomSeed::new(
        digest('d'),
        vec![Draw::from_trusted("00"), Draw::from_trusted("01")],
    );
    assert_ne!(seed_left, seed_more_draws);
}

#[test]
fn no_source_file_reaches_a_real_clock_or_entropy_source() {
    // The executable half of invariant 1/5. Comment lines may name them; code may not.
    let mut violations = Vec::new();
    for (path, body) in crate::src_files() {
        for line in crate::code_lines(&body) {
            for forbidden in [
                "SystemTime::now",
                "Instant::now",
                "thread_rng",
                "OsRng",
                "getrandom",
            ] {
                if line.contains(forbidden) {
                    violations.push(format!("{path}: {forbidden}"));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "nondeterminism:\n{}",
        violations.join("\n")
    );
}
