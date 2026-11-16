//! The R0 receipt's published contract: the schema, the golden pair, and the canonical vector (#1590).
//!
//! Three properties, each of which was missing and each of which a second producer needs:
//!
//! 1. **The schema is the type's, not a copy of it.** `receipt_schema()` derives the JSON Schema from
//!    `Receipt`; the checked-in `engine/schema/receipt.schema.json` is compared with it byte for byte,
//!    so a field added to the type without regenerating the artifact fails here. A hand-maintained
//!    schema is a second copy of a fact, and it drifts on the first field addition.
//! 2. **There is a document the engine accepts and documents it refuses**, one byte-level difference
//!    each, with the refusal named as a `kind()` — a variant, not a message, because a message is prose
//!    and prose is allowed to improve.
//! 3. **The canonical bytes can be reproduced without the engine.** The digest is over sorted keys and
//!    no whitespace, so the test builds that byte string by hand and compares it with the digest the
//!    engine computes: field order, absent-vs-empty, and `skip_serializing_if` all change the digest,
//!    and a producer in another language has to get all three right.
//!
//! Regenerate the schema after changing the type:
//!
//! ```bash
//! UPDATE_RECEIPT_SCHEMA=1 cargo test -p rigorix-engine --lib receipt
//! ```

use std::path::{Path, PathBuf};

use rigorix_engine::receipt::domain::receipt::Receipt;
use rigorix_engine::receipt::domain::refusal::ReceiptRefusal;
use rigorix_engine::receipt::{RECEIPT_SCHEMA_PATH, canonical_json, receipt_schema, receipt_schema_text, sha256_hex};

/// The identity the engine computed for the fixture's tree, taken from the *accepted* fixture.
///
/// It has to be a constant of the fixture set rather than a field of the document being checked: a
/// receipt verified against the workspace it names proves nothing, and a fixture that names a
/// different tree could never be refused if the identity came from itself.
fn engine_identity() -> String {
    let accepted: serde_json::Value =
        serde_json::from_str(&read(&schema_file("schema/receipt.json"))).expect("fixture is JSON");
    accepted
        .pointer("/workspace/value")
        .and_then(serde_json::Value::as_str)
        .expect("the accepted fixture names a tree")
        .to_string()
}

/// `engine/` — one level above `CARGO_MANIFEST_DIR` is the repository root, this is the crate root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn schema_file(path: &str) -> PathBuf {
    crate_root().join(path)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// One row of `engine/schema/manifest.tsv`.
struct Fixture {
    fixture: String,
    door: String,
    expected: String,
}

fn manifest() -> Vec<Fixture> {
    let text = read(&schema_file("schema/manifest.tsv"));
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() || line.starts_with("fixture\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        assert!(
            fields.len() >= 3,
            "schema/manifest.tsv: a row needs fixture, door, expected — got {line:?}"
        );
        rows.push(Fixture {
            fixture: fields[0].to_string(),
            door: fields[1].to_string(),
            expected: fields[2].to_string(),
        });
    }
    assert!(
        rows.len() >= 2,
        "schema/manifest.tsv lists {} row(s): a golden pair with no refusals is not a pair",
        rows.len()
    );
    rows
}

// ── 1. the schema is the type's ─────────────────────────────────────────────

#[test]
fn the_checked_in_schema_is_the_one_the_type_derives() {
    let generated = receipt_schema_text().expect("the derived schema serializes");
    let path = schema_file(RECEIPT_SCHEMA_PATH);

    if std::env::var("UPDATE_RECEIPT_SCHEMA").as_deref() == Ok("1") {
        std::fs::write(&path, &generated).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        panic!(
            "wrote {} — re-run without UPDATE_RECEIPT_SCHEMA to compare, and commit the file",
            path.display()
        );
    }

    let checked_in = read(&path);
    assert_eq!(
        checked_in, generated,
        "{} is not the schema `Receipt` derives. The type is the contract; regenerate with \
         UPDATE_RECEIPT_SCHEMA=1 and commit the difference (a field added to the type is a field a \
         producer has to know about).",
        path.display()
    );
}

#[test]
fn the_schema_identifies_the_version_the_engine_reads() {
    // `schemars` describes `schema_version` as an integer (it cannot know the value), so the version
    // a producer reads off this artifact is its `$id` — which therefore has to carry it, and the
    // accepted fixture has to agree, or the artifact would name one version and demonstrate another.
    let schema = receipt_schema().expect("the derived schema serializes");
    let id = schema
        .get("$id")
        .and_then(serde_json::Value::as_str)
        .expect("the schema has an $id");
    let version = rigorix_engine::receipt::domain::receipt::RECEIPT_SCHEMA_VERSION;
    assert!(
        id.contains(&format!("receipt-{version}.json")),
        "the schema's $id is {id:?}, and this engine reads schema version {version}"
    );
    let fixture: serde_json::Value =
        serde_json::from_str(&read(&schema_file("schema/receipt.json"))).expect("fixture is JSON");
    assert_eq!(
        fixture.get("schema_version").and_then(serde_json::Value::as_u64),
        Some(u64::from(version)),
        "the accepted fixture must carry the version the engine reads"
    );
}

#[test]
fn the_schema_closes_its_member_sets_because_the_type_does() {
    // A signed declaration must not silently ignore a member the author wrote: `Receipt` and the six
    // types it is made of carry `#[serde(deny_unknown_fields)]`, and the schema has to say the same
    // thing or a producer validating against it would accept a document the engine refuses.
    let schema = receipt_schema().expect("the derived schema serializes");
    assert_eq!(
        schema.pointer("/additionalProperties"),
        Some(&serde_json::Value::Bool(false)),
        "the receipt schema must close its member set: the type refuses an unknown member, so a \
         schema that permits one describes a different contract"
    );
    let definitions = schema.get("$defs").and_then(serde_json::Value::as_object);
    let definitions = definitions.expect("the derived schema has definitions");
    for name in [
        "Workspace",
        "Sandbox",
        "DeclaredEffect",
        "Expectation",
        "ReceiptStep",
        "BudgetDeclared",
        "QualityClaim",
    ] {
        assert_eq!(
            definitions
                .get(name)
                .and_then(|d| d.get("additionalProperties")),
            Some(&serde_json::Value::Bool(false)),
            "`{name}` closes its members in the type, so it must close them in the schema"
        );
    }
}

#[test]
fn an_unknown_member_is_refused_by_the_door_a_document_meets() {
    // The schema's claim, checked against the parser rather than against the schema: a document with
    // `roles` — the field v2 removed — is refused, not parsed-and-ignored.
    let document = read(&schema_file("schema/receipt.json"));
    let mut value: serde_json::Value = serde_json::from_str(&document).expect("fixture is JSON");
    value["roles"] = serde_json::json!([]);
    let with_roles = serde_json::to_string(&value).expect("serializes");
    let kinds: Vec<&str> = Receipt::from_document(&with_roles)
        .expect_err("an unknown member is refused")
        .iter()
        .map(ReceiptRefusal::kind)
        .collect();
    assert!(
        kinds.contains(&"malformed_document"),
        "an unknown member produced {kinds:?}: it is the parser's answer, carried in this module's \
         vocabulary"
    );
}

// ── 2. the golden pair ──────────────────────────────────────────────────────

fn refused_kind(document: &str, door: &str) -> Vec<String> {
    match door {
        "from_document" => match Receipt::from_document(document) {
            Ok(_) => Vec::new(),
            Err(refusals) => refusals.iter().map(|r| r.kind().to_string()).collect(),
        },
        "verify" => match Receipt::from_document(document) {
            Err(refusals) => refusals.iter().map(|r| r.kind().to_string()).collect(),
            Ok(receipt) => {
                let identity = engine_identity();
                match rigorix_engine::receipt::verify(&receipt, &identity) {
                    Ok(_) => Vec::new(),
                    Err(refusals) => refusals.iter().map(|r| r.kind().to_string()).collect(),
                }
            }
        },
        other => panic!("schema/manifest.tsv: unknown door {other:?}"),
    }
}

#[test]
fn every_manifest_row_meets_the_door_it_names() {
    let rows = manifest();
    assert!(
        rows.iter().any(|row| row.expected == "accepted"),
        "schema/manifest.tsv has no accepted row: a fixture set that refuses everything proves the \
         refusals and not the format"
    );

    for row in rows {
        let path = schema_file(&format!("schema/{}", row.fixture));
        let document = read(&path);
        let kinds = refused_kind(&document, &row.door);
        if row.expected == "accepted" {
            assert!(
                kinds.is_empty(),
                "{} is the accepted fixture and the engine refused it: {kinds:?}",
                row.fixture
            );
            continue;
        }
        assert!(
            kinds.iter().any(|kind| kind == &row.expected),
            "{} through `{}` produced {kinds:?}, and the manifest says `{}`",
            row.fixture,
            row.door,
            row.expected
        );
    }
}

#[test]
fn the_accepted_fixture_is_a_receipt_and_not_merely_parseable() {
    // `from_document` answering `Ok` is a parse; the declaration has to be *usable*: its digest
    // computes, its steps are named, and its declared effects pair their paths with digests.
    let document = read(&schema_file("schema/receipt.json"));
    let receipt = Receipt::from_document(&document).expect("the accepted fixture parses");
    assert!(!receipt.steps.is_empty(), "the accepted fixture declares no step");
    assert!(
        receipt.digest().is_ok(),
        "the accepted fixture's canonical form does not serialize, so its digest cannot be taken"
    );
    for step in &receipt.steps {
        assert_eq!(
            step.declared_effects.paths.len(),
            step.declared_effects.content_digests.len(),
            "step {} declares {} path(s) and {} digest(s)",
            step.id,
            step.declared_effects.paths.len(),
            step.declared_effects.content_digests.len()
        );
    }
}

// ── 3. the canonical vector, reproduced by hand ─────────────────────────────

#[test]
fn the_canonical_bytes_of_the_fixture_are_reproducible_by_hand() {
    let document = read(&schema_file("schema/receipt.json"));
    let receipt = Receipt::from_document(&document).expect("the accepted fixture parses");

    // Written out here, in the documented form — sorted object keys, no insignificant whitespace —
    // rather than produced by the engine. This is the part a producer in another language needs and
    // the part most likely to be wrong: `set_digest` is absent because it is `None` and
    // `skip_serializing_if`, `fixture_digests` is present and empty because `#[serde(default)]` does
    // not skip, and the steps' keys are sorted inside each object.
    let by_hand = concat!(
        r#"{"budget":{"max_iterations":3,"max_tokens":20000},"#,
        r#""execution_id":"018f0a2c-7d3e-7c1a-9d2b-0f4a6c8e2b10","#,
        r#""fixture_digests":[],"#,
        r#""quality_required":{"level":"targeted_tests","source":"caller"},"#,
        r#""sandbox":{"image_digest":"sha256:a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1","regime":"container"},"#,
        r#""schema_version":2,"#,
        r#""steps":[{"declared_effects":{"content_digests":["d3f1c0a1b2c3d4e5f60718293a4b5c6d7e8f9012a3b4c5d6e7f8091a2b3c4d5e"],"paths":["out.txt"]},"depends_on":[],"expectation":{"description":"out.txt contains hello"},"id":"write-report","parameters":{"content":"hello\n","path":"out.txt"},"requires_approval":false,"tool":"file_write"}],"#,
        r#""workspace":{"kind":"commit_tree","value":"4b825dc642cb6eb9a060e54bf8d69288fbee4904"}}"#,
    );
    // The recorded digest of those bytes. A producer in another language checks its own
    // canonicalisation against a constant, not against a running engine.
    const RECORDED_DIGEST: &str = "8449cea10313baf8be0a2c647c10f129aebb0c3ffae832c610c63345d3785876";

    let engine_bytes = canonical_json(&receipt).expect("the receipt has a canonical form");
    let engine_text = String::from_utf8(engine_bytes.clone()).expect("canonical bytes are ASCII JSON");

    assert_eq!(
        engine_text, by_hand,
        "the canonical form is not the documented ordering: the engine wrote the first string and a \
         hand-built producer the second"
    );
    assert_eq!(
        sha256_hex(engine_text.as_bytes()),
        RECORDED_DIGEST,
        "the recorded vector changed: a producer that canonicalises correctly and holds this constant \
         would disagree with the engine"
    );
    assert_eq!(
        receipt.digest().expect("digest").as_str(),
        RECORDED_DIGEST,
        "`Receipt::digest` is SHA-256 over the canonical bytes, lowercase hex"
    );
}

#[test]
fn a_refusal_names_its_kind_so_a_producer_can_branch_without_matching_prose() {
    // The manifest's `expected` column holds kinds. This pins that every kind is non-empty and stable
    // enough to be compared — the alternative is asserting on `Display`, which this repository treats
    // as prose (`ReceiptRefusal::kind` exists precisely so callers do not).
    let refusal = ReceiptRefusal::SandboxMissing;
    assert_eq!(refusal.kind(), "sandbox_missing");
    let document = read(&schema_file("schema/refused/schema_version.json"));
    let kinds: Vec<&str> = Receipt::from_document(&document)
        .expect_err("version 1 is not a version this engine reads")
        .iter()
        .map(ReceiptRefusal::kind)
        .collect();
    assert!(
        kinds.contains(&"schema_version"),
        "a version-1 document produced {kinds:?}"
    );
}