//! #1658 item 2 — a file the engine writes **about itself** is readable by its owner only.
//!
//! The item was "reported, not reproduced", so it was carried as a measurement rather than a change.
//! Reproduced: with `umask 022`, `std::fs::write` produces **0644** and `create_dir_all` **0755**, so every
//! envelope under `.rigorix/audit/` — which records the planning prompt, the file paths and the findings —
//! was readable by every account on the host. The agent's own writes were never in that state; `fs_at`
//! opens them 0600.
//!
//! These tests fail on the old code for the reason the issue gives, and they are the reproduction the
//! "measure first" rule asked for.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use rigorix_engine::audit::domain::AuditEnvelope;
use rigorix_engine::audit::infrastructure::LocalAuditEnvelopeRepository;
use rigorix_engine::audit::infrastructure::repository::AuditEnvelopeRepository;

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path)
        .unwrap_or_else(|error| panic!("{} has no metadata: {error}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

fn envelope(id: uuid::Uuid) -> AuditEnvelope {
    serde_json::from_value(serde_json::json!({
        "execution_id": id,
        "timestamp": "2026-11-24T00:00:00Z",
        "template_id": "modes",
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
    .expect("a minimal envelope")
}

#[tokio::test]
async fn an_audit_envelope_is_written_owner_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = LocalAuditEnvelopeRepository::new(dir.path().join("audit"));
    repository
        .save(&envelope(uuid::Uuid::new_v4()))
        .await
        .expect("save");

    let written = std::fs::read_dir(dir.path().join("audit"))
        .expect("the audit directory")
        .flatten()
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .expect("the envelope");
    assert_eq!(
        mode(&written.path()),
        0o600,
        "an envelope records the prompt and the paths; another account must not read it"
    );
    assert_eq!(
        mode(&dir.path().join("audit")),
        0o700,
        "and the directory that holds them is closed too"
    );
}

#[cfg(test)]
mod declared_measurement {
    /// The measurement the issue asked for, kept beside the fix so a reader can re-run it rather than
    /// trust the sentence: `std::fs::write` under `umask 022` is 0644.
    ///
    /// It is a test of the **platform**, not of this crate, so it asserts the shape of the old behaviour
    /// only when the umask allows it — on a host whose umask already removes group/other bits, the premise
    /// is absent and the assertion would be about the host, not the code.
    #[test]
    fn the_old_creation_path_was_world_readable_under_the_usual_umask() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("plain.json");
        std::fs::write(&path, "{}").expect("write");
        let created = std::fs::metadata(&path).expect("mode").permissions().mode() & 0o777;
        let umask_removes_group_and_other = created & 0o077 == 0;
        if umask_removes_group_and_other {
            eprintln!(
                "umask already closes the file ({created:o}); the premise is absent on this host"
            );
            return;
        }
        assert_eq!(
            created, 0o644,
            "the reproduction: plain creation is group/other readable"
        );
    }
}
