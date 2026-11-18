//! The attestation store has a read side, and a location rule belongs to the store (#1628).
//!
//! Before this, `FileAttestationStore` had `put` and no `get`, so the only way to obtain a sealed
//! attestation was to know where the store had put it — and `receipt::api::run_receipt` did exactly
//! that, spelling `<state dir>/attestations/<id>.json` in a second file. `FileReceiptStore` already
//! had `get`, so the declaration was readable through its store and the observation was not.
//!
//! Each test builds its own state directory and seal, so the expectation is owned here rather than
//! read from the store: a test whose cases come from the implementation proves only that the
//! implementation agrees with itself.
//!
//! Gate: the receipt module is engine-only (it reads `quality_gates`, outside the portable slice), so
//! this module carries the same `#[cfg(not(feature = "portable"))]` gate as its siblings.

use std::path::Path;

use rigorix_engine::quality_gates::domain::level::QualityLevel;
use rigorix_engine::receipt::infrastructure::attestation_store::{
    AttestationStoreError, FileAttestationStore, StoredAttestation,
};
use rigorix_engine::receipt::{
    AttestationBuilder, BudgetDeclared, DeclaredEffect, Expectation, Receipt, ReceiptRefusal,
    ReceiptStep, Sandbox, SandboxRegime, Workspace, WorkspaceKind, verify,
};
use uuid::Uuid;

/// A state directory that is **not** inside the workspace — the rule ADR-024 §6 makes, so the
/// fixture is on the permitted side of it and a test that needs the refusal moves it deliberately.
struct Fixture {
    workspace: std::path::PathBuf,
    state: std::path::PathBuf,
    key: String,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "rigorix-attestation-store-{name}-{}",
            std::process::id()
        ));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("a clean fixture root");
        }
        let workspace = root.join("workspace");
        let state = root.join("state");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(&state).expect("state");
        Self {
            workspace,
            state,
            // The store keys HMAC-SHA256; any non-empty key is a valid one, and the fixture picks a
            // value of its own so the test never reads the key from the environment a runner happens
            // to have.
            key: "fixture-key-#1628".to_string(),
        }
    }

    fn store(&self) -> FileAttestationStore {
        FileAttestationStore::new(
            self.state.clone(),
            self.workspace.clone(),
            Some(self.key.clone()),
        )
    }

    fn store_with_state(&self, state: impl Into<std::path::PathBuf>) -> FileAttestationStore {
        FileAttestationStore::new(state, self.workspace.clone(), Some(self.key.clone()))
    }
}

fn step(id: &str, depends_on: &[&str]) -> ReceiptStep {
    ReceiptStep {
        id: id.to_string(),
        tool: "file_write".to_string(),
        parameters: serde_json::json!({ "path": format!("{id}.txt") }),
        depends_on: depends_on.iter().map(|d| (*d).to_string()).collect(),
        declared_effects: DeclaredEffect {
            paths: vec![format!("{id}.txt")],
            content_digests: vec![format!("digest-of-{id}")],
        },
        expectation: Expectation {
            description: format!("{id} writes its file"),
            quality_level: None,
        },
        requires_approval: false,
        timeout_secs: None,
    }
}

/// The attestation for `receipt` — and note there is no `execution_id` argument: the builder takes it
/// from the receipt, which is why a fixture that passed one separately would describe a record the
/// store never wrote (the first version of this test did exactly that and the seal went to `<nil>`).
fn attestation_for(receipt: &Receipt) -> rigorix_engine::receipt::Attestation {
    // Built from a *verified* receipt, the only way the builder starts: the store test needs an
    // attestation, not a shortcut past the gate.
    let verified = verify(receipt, &workspace_of(receipt))
        .expect("the fixture receipt is sound, so `from_verified` is reachable");
    AttestationBuilder::from_verified(&verified).build(0, 0)
}

/// The workspace value the fixture receipt declares, read from the receipt so the two cannot drift.
fn workspace_of(receipt: &Receipt) -> String {
    receipt.workspace.value.clone()
}

/// The receipt's workspace identity is irrelevant to a store test: the store writes what it is
/// handed and checks where, not what.
fn receipt(execution_id: Uuid) -> Receipt {
    Receipt::new(
        execution_id,
        Workspace {
            kind: WorkspaceKind::CommitTree,
            value: "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string(),
        },
        Sandbox {
            regime: SandboxRegime::Container,
            image_digest: "sha256:1111".to_string(),
        },
        vec![step("a", &[])],
        BudgetDeclared {
            max_iterations: 10,
            max_tokens: 1000,
        },
        QualityLevel::MergeReady,
    )
    .expect("the fixture carries no credentials")
}

fn sealed(fixture: &Fixture, execution_id: Uuid) -> StoredAttestation {
    let store = fixture.store();
    let mut attestation = attestation_for(&receipt(execution_id));
    store.put(&mut attestation).expect("the seal is written")
}

#[test]
fn a_sealed_attestation_is_read_back_through_the_store() {
    let fixture = Fixture::new("round-trip");
    let execution_id = Uuid::from_u128(0x1628_0001);
    let written = sealed(&fixture, execution_id);

    let read = fixture
        .store()
        .get(&execution_id.to_string())
        .expect("the record the store wrote is readable through the store");

    assert_eq!(
        read.digest, written.digest,
        "the reader and the writer describe one record in one vocabulary"
    );
    assert_eq!(read.path, written.path, "one location rule, not two");
    assert_eq!(
        read.bytes, written.bytes,
        "the bytes a verifier receives are the bytes the store wrote — a `get` that re-serialised \
         would make the sealed artifact unavailable to `verify_attestation`"
    );
    let verified = rigorix_engine::receipt::verify_attestation(&read.bytes, fixture.key.as_bytes())
        .expect("the read-back bytes verify under the key the store signed with");
    assert_eq!(verified.attestation().execution_id(), execution_id);
}

#[test]
fn an_unknown_execution_is_a_typed_miss_rather_than_a_store_failure() {
    let fixture = Fixture::new("miss");
    let store = fixture.store();

    let error = store
        .get(&Uuid::from_u128(0x1628_0002).to_string())
        .expect_err("nothing has been sealed for that id");

    match error {
        AttestationStoreError::UnknownExecution { execution_id } => {
            assert_eq!(execution_id, Uuid::from_u128(0x1628_0002).to_string());
        }
        other => {
            panic!("a caller must be able to tell 'never sealed' from 'cannot read'; got {other:?}")
        }
    }
}

#[test]
fn a_state_dir_inside_the_workspace_is_refused_on_the_read_side_too() {
    let fixture = Fixture::new("inside");
    let execution_id = Uuid::from_u128(0x1628_0003);
    // Sealed where it belongs, so the record *exists* and the refusal can only be about where it is.
    let written = sealed(&fixture, execution_id);
    assert!(
        written.path.starts_with(&fixture.state),
        "the fixture seals outside the workspace"
    );

    // Now read through a store whose state directory is inside the workspace: the same file name,
    // resolved from the place the writer refuses. A reader that trusted it would turn a refused
    // write into a readable record.
    let inside = fixture.workspace.join(".rigorix").join("state");
    std::fs::create_dir_all(&inside).expect("the workspace-local state directory");
    let error = fixture
        .store_with_state(inside)
        .get(&execution_id.to_string())
        .expect_err("the location rule applies to a reader as well as a writer");

    match error {
        AttestationStoreError::Refused(ReceiptRefusal::StateDirInsideWorkspace { .. }) => {}
        other => panic!("expected the location rule's own refusal; got {other:?}"),
    }
}

#[test]
fn an_execution_id_that_is_not_a_file_stem_is_refused_rather_than_sanitised() {
    let fixture = Fixture::new("traversal");
    let store = fixture.store();

    for candidate in ["../escape", "a/b", "", "."] {
        let error = store
            .get(candidate)
            .expect_err("an id is a file stem, not a path");
        assert!(
            matches!(error, AttestationStoreError::InvalidExecutionId { .. }),
            "{candidate:?} must be refused by name, got {error:?}"
        );
    }
}

#[test]
fn the_location_rule_is_written_once() {
    // Acceptance criterion, asserted rather than trusted: no production file outside the store spells
    // the record's directory. The scan excludes this file, the store itself, and the changelog — a
    // scan matching its own subject's text is how #1548's guard reported itself.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for entry in walk(&root) {
        let text = std::fs::read_to_string(&entry).unwrap_or_default();
        if entry.ends_with("receipt/infrastructure/attestation_store.rs") {
            continue;
        }
        // A test that asserts where the seal lands *is* the expectation, and it is the thing that
        // notices a layout change. It is not a second producer of the rule.
        let path = entry.to_string_lossy();
        if path.contains("/tests/") || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            // The production half only: a doc comment that *names* the directory is not a call site,
            // and a guard that cannot tell them apart is muted within a week (#1627's rule, applied
            // one issue early because this scan hit it first — three of its four findings were prose).
            let production = line.trim_start();
            if production.starts_with("//") {
                continue;
            }
            if line.contains("attestations/") || line.contains("\"attestations\"") {
                offenders.push(format!("{}:{}: {line}", entry.display(), index + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the record's directory belongs to the store:\n{}",
        offenders.join("\n")
    );
}

/// Every `.rs` file under `dir`, recursively. `std` has no walker and the tree has no dev-dependency
/// for one; this is twelve lines and it cannot silently skip a directory.
fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files
}
