//! The R0 receipt's JSON Schema — **derived from the type**, never maintained beside it (#1590).
//!
//! ADR-024 splits one run between a host that writes R0 (the declaration) and the engine that
//! verifies it, executes and signs R1. The engine's answers were complete — [`super::Receipt::from_document`],
//! the typed refusals, `verify` — and the *contract* was only readable as Rust. A second producer had
//! three ways to learn it, all bad: read this crate's source, guess from the canonical bytes it gets
//! back, or submit and be refused. A producer with no schema re-derives the format, and the first
//! divergence is discovered as a refusal at run time instead of as a failed build.
//!
//! The schema is generated, not written: `schemars` derives it from `Receipt` and the six types it is
//! made of, so a field added without a decision in ADR-024 §4 still appears here — the schema describes
//! the type that will parse the document, which is the only thing a producer can rely on. The
//! checked-in copy lives at `engine/schema/receipt.schema.json`, and
//! `tests/unit/receipt/receipt_schema_test.rs` fails when the two disagree.
//!
//! It ships in the crate (`cargo package` includes `schema/`) and in the repository, so a consumer can
//! fetch it without building anything.

use serde_json::Value;

use super::receipt::{RECEIPT_SCHEMA_VERSION, Receipt};

/// Where the checked-in copy lives, relative to the crate root.
///
/// The test that compares them reads this constant, so a rename is one edit and the comparison cannot
/// quietly stop pointing at a file that no longer exists.
pub const RECEIPT_SCHEMA_PATH: &str = "schema/receipt.schema.json";

/// The JSON Schema of the R0 receipt document.
///
/// The `$id` and `title` are added here rather than derived, because they name *this* artifact — the
/// receipt document of this engine version — and a schema whose identity is generated from a type name
/// changes when a module is renamed.
///
/// # Errors
/// The schema's own serialization error, as a string. A derived schema always serializes, so a failure
/// here is a defect in the derive rather than a case a caller branches on.
pub fn receipt_schema() -> Result<Value, String> {
    let schema = schemars::schema_for!(Receipt);
    let mut value = serde_json::to_value(&schema).map_err(|e| format!("receipt schema: {e}"))?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "$id".to_string(),
            Value::String(format!(
                "https://github.com/arman-jalili/rigorix-oss/engine/schema/receipt-{RECEIPT_SCHEMA_VERSION}.json"
            )),
        );
        object.insert(
            "title".to_string(),
            Value::String("R0 execution receipt".to_string()),
        );
        object.insert(
            "description".to_string(),
            Value::String(
                "The declaration a host writes before an execution (ADR-024 §4). The engine parses it \
                 with `Receipt::from_document`, verifies it against the workspace identity it computed, \
                 and refuses it with the variants named in `ReceiptRefusal`."
                    .to_string(),
            ),
        );
    }
    Ok(value)
}

/// The schema as the bytes the checked-in file holds: pretty-printed, key order as the generator emits.
///
/// # Errors
/// [`receipt_schema`]'s error, and the serializer's.
pub fn receipt_schema_text() -> Result<String, String> {
    let value = receipt_schema()?;
    let mut text = serde_json::to_string_pretty(&value).map_err(|e| format!("receipt schema: {e}"))?;
    text.push('\n');
    Ok(text)
}