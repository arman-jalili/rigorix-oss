//! Dedicated serialization + hydration tests for state_persistence (GAP-M-01).
//!
//! Covers:
//! - serialize/deserialize round trips (full state incl. node_states,
//!   approved, exec_node_states)
//! - legacy pre-GAP-3 files (missing additive fields) hydrate with defaults
//! - legacy `approved: Vec<Uuid>` invalidation on hydrate (migration rule)

use crate::state_persistence::domain::{ExecutionState, ExecutionStatus, NodeState};
use uuid::Uuid;

fn sample_state() -> ExecutionState {
    // GAP-M-15: exec_node_states is the persisted node-state vocabulary.
    let mut state = ExecutionState::new(Uuid::new_v4(), "hash-v1".to_string());
    let node_id = Uuid::new_v4();
    let mut exec = crate::execution_engine::domain::NodeExecutionState::new(node_id, "sample");
    exec.status = crate::execution_engine::domain::NodeStatus::Completed;
    exec.last_duration_ms = Some(10);
    let mut map = std::collections::HashMap::new();
    map.insert(node_id, exec);
    state.exec_node_states = Some(map);
    state
}

#[test]
fn test_full_state_serialize_deserialize_round_trip() {
    let mut state = sample_state();
    let node_id = *state
        .exec_node_states
        .as_ref()
        .expect("sample_state seeds exec")
        .keys()
        .next()
        .unwrap();
    state.status = ExecutionStatus::Completed;
    state.completed_at = Some(chrono::Utc::now());
    state.approved = vec![node_id];

    let json = serde_json::to_string(&state).unwrap();
    // GAP-M-15: coarse node_states is NOT serialized (derived view) — the
    // persisted representation is exec_node_states.
    // NB: "exec_node_states" contains the substring — check the exact key form.
    assert!(
        !json.contains("\"node_states\":"),
        "coarse map must not be persisted"
    );
    assert!(
        json.contains("exec_node_states"),
        "exec map must be persisted"
    );
    let mut back: ExecutionState = serde_json::from_str(&json).unwrap();

    assert_eq!(back.execution_id, state.execution_id);
    assert_eq!(back.status, ExecutionStatus::Completed);
    assert_eq!(back.symbol_graph_hash, "hash-v1");
    assert_eq!(back.approved, vec![node_id]);
    // Coarse is empty until derived from the canonical exec map.
    assert!(back.node_states.is_empty());
    back.derive_node_states_from_exec();
    assert_eq!(back.node_states.len(), 1);
    let node = back.node_states.get(&node_id).unwrap();
    assert_eq!(
        node.status,
        crate::state_persistence::domain::NodeStatus::Completed
    );
    assert_eq!(node.duration_ms, Some(10));
    assert!(back.completed_at.is_some());
}

#[test]
fn test_legacy_state_file_hydrates_with_defaults() {
    // A pre-GAP-3 state file has only the original fields — the additive
    // fields (graph, approved, exec_node_states) must default on hydrate.
    let legacy_json = format!(
        r#"{{
            "execution_id": "{}",
            "status": "Running",
            "started_at": "2026-01-01T00:00:00Z",
            "completed_at": null,
            "node_states": {{}},
            "symbol_graph_hash": "legacy-hash"
        }}"#,
        Uuid::new_v4()
    );
    let state: ExecutionState = serde_json::from_str(&legacy_json).unwrap();
    assert!(state.graph.is_none());
    assert!(state.approved.is_empty());
    assert!(state.exec_node_states.is_none());
}

#[test]
fn test_legacy_approved_invalidated_on_hydrate_migration_rule() {
    // GAP-M-01 migration rule: approvals persisted under the LEGACY scheme
    // (no exec_node_states) were granted against the old coarse node_states
    // vocabulary — they are invalid on hydrate and must be cleared.
    let legacy_with_approvals = format!(
        r#"{{
            "execution_id": "{}",
            "status": "Pending",
            "started_at": "2026-01-01T00:00:00Z",
            "completed_at": null,
            "node_states": {{}},
            "symbol_graph_hash": "legacy-hash",
            "approved": ["{}"]
        }}"#,
        Uuid::new_v4(),
        Uuid::new_v4()
    );
    let mut state: ExecutionState = serde_json::from_str(&legacy_with_approvals).unwrap();
    assert!(
        !state.approved.is_empty(),
        "legacy payload carries approvals"
    );
    state.invalidate_legacy_approvals();
    assert!(
        state.approved.is_empty(),
        "legacy approvals invalidated on hydrate"
    );
}

#[test]
fn test_gap3_state_preserves_approved_on_hydrate() {
    // GAP-3+ files carry exec_node_states — their approved set is preserved.
    let mut state = sample_state();
    let node_id = *state
        .exec_node_states
        .as_ref()
        .expect("sample_state seeds exec")
        .keys()
        .next()
        .unwrap();
    state.approved = vec![node_id];
    state.exec_node_states = Some(std::collections::HashMap::new());

    let json = serde_json::to_string(&state).unwrap();
    let mut back: ExecutionState = serde_json::from_str(&json).unwrap();
    back.invalidate_legacy_approvals();
    assert_eq!(back.approved, vec![node_id], "GAP-3 approvals are kept");
}

#[test]
fn test_node_state_round_trip() {
    let node = NodeState {
        node_id: Uuid::new_v4(),
        status: crate::state_persistence::domain::NodeStatus::Completed,
        output: Some("done".to_string()),
        error: None,
        retries: 1,
        duration_ms: Some(5),
        started_at: Some(chrono::Utc::now()),
        completed_at: Some(chrono::Utc::now()),
    };
    let json = serde_json::to_string(&node).unwrap();
    let back: NodeState = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.status,
        crate::state_persistence::domain::NodeStatus::Completed
    );
    assert_eq!(back.output, Some("done".to_string()));
}
