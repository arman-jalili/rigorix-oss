//! # #1609 — a resumed envelope reports the run's attribution, not the resumer's
//!
//! An approval pause writes an `ExecutionState`; the resume path re-sends an audit envelope for the
//! **same execution**. Before this fix that second envelope carried `repository: None` and
//! `author: None` as literals (`mcp/src/host/mod.rs`), so the audit chain held two envelopes for one
//! `execution_id` disagreeing in exactly the fields that answer "who did this, on what repository" —
//! while the run door had filled both from the tool parameters.
//!
//! The tests below own their expectations: the pair comes from the state the pause wrote, and the
//! assertions are **equality** with what the original envelope carried, because "present" would pass
//! for a value that had been re-derived from the resumer's environment (`git config user.email`
//! answers "who is running this process now", which after a handoff is a different person).
//!
//! The host's choice between those sources is a private function, so its behaviour is pinned here as
//! the shape of the source: the resume arm must call `resume_attribution(&state)` and must not write
//! either field as a literal. A test that only checked the helper would pass while the arm stopped
//! calling it.

use rigorix_engine::state_persistence::domain::ExecutionState;

/// The attribution a run was triggered with, as the run door supplies it (#1609).
fn triggered_with() -> (Option<String>, Option<String>) {
    (
        Some("acme/widgets".to_string()),
        Some("operator@acme.example".to_string()),
    )
}

/// The paused state the approval path writes, built the way `run_from_template` builds it: the
/// attribution is recorded at pause time, from the run's own input.
fn paused_state_with_attribution() -> ExecutionState {
    let mut state = ExecutionState::new(uuid::Uuid::new_v4(), "symbol-graph-hash".to_string());
    let (repository, author) = triggered_with();
    state.repository = repository;
    state.author = author;
    state
}

#[test]
fn a_paused_state_carries_the_attribution_a_resume_needs() {
    // Equality, not presence: the pair the resume re-sends is the pair the run was triggered with.
    let (repository, author) = triggered_with();
    let state = paused_state_with_attribution();

    let json = serde_json::to_string(&state).expect("a paused state serializes");
    let back: ExecutionState = serde_json::from_str(&json).expect("and reads back");

    assert_eq!(back.repository, repository);
    assert_eq!(back.author, author);
    assert_eq!(
        (back.repository, back.author),
        (repository, author),
        "the fields a resume reads must survive the state file it reads them from"
    );
}

#[test]
fn a_state_file_written_before_the_fields_loads_with_none() {
    // Additive and serde-defaulted: an old file must still load, and its absence must read as "the
    // run recorded no attribution" rather than as a parse failure or an invented value. That is the
    // distinction the issue asks for — an unrecorded value is not the same fact as a resume path
    // that declined to carry one.
    let state = paused_state_with_attribution();
    let mut json: serde_json::Value =
        serde_json::to_value(&state).expect("a paused state serializes to JSON");
    let object = json
        .as_object_mut()
        .expect("the state is a JSON object, not an array or scalar");
    object.remove("repository");
    object.remove("author");

    let back: ExecutionState =
        serde_json::from_value(json).expect("a state file without the fields still loads");
    assert_eq!(back.repository, None);
    assert_eq!(back.author, None);
}

#[test]
fn the_resume_arm_takes_the_attribution_from_the_state_and_writes_no_literal() {
    // The resume path's envelope is built inline inside one large match arm, so the wiring is pinned
    // by reading the source: the arm must call `resume_attribution(&state)`, and the
    // `BuildEnvelopeInput` it builds must carry that pair.
    //
    // The subject is the **envelope**, not every `author: None,` in the file: the issue's acceptance
    // grep ("the remaining hits are tests") does not hold on this tree — `build_real_engine` registers
    // seed templates with `author: None`, which is `Template::author`, a different field that happens
    // to share a spelling. An assertion over bare field names would fail on that, and would pass on an
    // envelope that had gone blind in some other way.
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/host/mod.rs"))
        .expect("the host module is part of this crate");

    let binding = "let attribution = resume_attribution(&state);";
    let bound = source
        .find(binding)
        .unwrap_or_else(|| panic!("the resume arm must bind the state's attribution: `{binding}`"));
    let envelope_at = source[bound..]
        .find("BuildEnvelopeInput {")
        .map(|offset| bound + offset)
        .expect("the attribution is bound next to the envelope it fills");
    let envelope = &source[envelope_at..];
    let body = &envelope[..envelope
        .find("})")
        .expect("the envelope literal closes with `})`")];

    assert!(
        body.contains("repository: attribution.0.clone(),"),
        "the resumed envelope does not carry the run's repository (#1609)"
    );
    assert!(
        body.contains("author: attribution.1.clone(),"),
        "the resumed envelope does not carry the run's author (#1609)"
    );
    for literal in ["repository: None,", "author: None,"] {
        assert!(
            !body.contains(literal),
            "the resumed envelope writes `{literal}` — the literal this issue is about"
        );
    }
}
