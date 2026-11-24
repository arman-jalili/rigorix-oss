//! #1658 (retention) — the bound is enforced by the store, and the oldest go first.
//!
//! The audit's finding: `.rigorix/audit/` grew one file per run, forever; `prune` existed and its
//! only call sites were inside the `#[cfg(test)]` module. With an open `/rpc` a loop of `run_command`
//! steps grew the server host's disk without bound.

use rigorix_engine::audit::domain::AuditEnvelope;
use rigorix_engine::audit::infrastructure::LocalAuditEnvelopeRepository;
use rigorix_engine::audit::infrastructure::repository::AuditEnvelopeRepository;

/// Build the envelope the store's own test builds — read from the type, not from a JSON shape, so a
/// new required field fails here rather than being silently defaulted.
fn envelope(id: uuid::Uuid) -> AuditEnvelope {
    let mut envelope: AuditEnvelope = serde_json::from_value(serde_json::json!({
        "execution_id": id,
        "timestamp": "2026-11-24T00:00:00Z",
        "template_id": "retention",
        "planning_hash": "abc123",
        "events": [],
        "total_tokens": 0,
        "duration_ms": 0,
        "file_paths": [],
        "scoring_results": {},
        "approval_events": [],
        "sequence_policy_findings": [],
        "requirement_findings": [],
        "precondition_findings": [],
        "scope_violations": [],
        "evidence_degraded": false,
    }))
    .expect("a minimal envelope deserializes");
    envelope.execution_id = id;
    envelope
}

#[tokio::test]
async fn the_store_removes_the_oldest_envelopes_past_the_bound() {
    let dir = tempfile::tempdir().expect("tempdir");
    // An explicit bound, not an environment variable: a test that mutates the process environment
    // races every other test that reads it, and the failure lands in whichever test loses.
    let repository = LocalAuditEnvelopeRepository::with_max_envelopes(dir.path().to_path_buf(), 3);

    // Six writes, spaced by mtime so "oldest" is well defined even on a coarse filesystem. The bound
    // is enforced on every save, so the survivors are the **last three** — asserting only "the first
    // one went" would pass for a store that deletes the wrong end.
    let mut written = Vec::new();
    for _ in 0..6 {
        let id = uuid::Uuid::new_v4();
        repository.save(&envelope(id)).await.expect("save");
        written.push(id);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }

    let files = std::fs::read_dir(dir.path())
        .expect("read dir")
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .count();
    assert_eq!(files, 3, "the bound is a count and it holds");

    for (index, id) in written.iter().enumerate() {
        let present = repository
            .find_by_execution_id(id)
            .await
            .expect("find")
            .is_some();
        let expected = index >= 3;
        assert_eq!(
            present, expected,
            "envelope {index} of 6: presence must be exactly the last three (the oldest were removed)"
        );
    }
}
