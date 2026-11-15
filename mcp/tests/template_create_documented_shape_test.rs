//! The create door stamps the host's clock, and the read door does not (#1563).
//!
//! Two entry points had opposite answers to one contract: the TOML path stamped `created_at` /
//! `updated_at` at the moment of reading, while the JSON create path demanded the caller supply them.
//! A client sending the shape the SDK documents (`{name, description, steps}`) was refused with
//! `missing field 'created_at'`, and one that invented timestamps had its invented clock stored as the
//! host's. The type kept them required so a document read back from disk cannot lack them — which is
//! the distinction these tests pin in both directions.

use chrono::{DateTime, TimeZone, Utc};
use rigorix_mcp::template_tools::domain::value::{CreateTemplateInput, PlanTemplate};

/// The SDK's documented request body, with nothing added: no timestamps, no version, no tags.
fn documented_plan() -> serde_json::Value {
    serde_json::json!({
        "name": "weekly-audit",
        "description": "Read the register and report what moved",
        "steps": [{
            "name": "read-register",
            "description": "Read the register",
            "tool": "file_read",
            "parameters": { "path": "engine/.pi/architecture/sink-register.tsv" },
            "requires_approval": false
        }]
    })
}

fn millis(at: &DateTime<Utc>) -> i64 {
    at.timestamp_millis()
}

#[test]
fn the_documented_payload_creates_a_template_stamped_with_the_hosts_clock() {
    let before = Utc::now();
    let template = PlanTemplate::from_create_json(documented_plan())
        .expect("the documented shape is accepted");
    let after = Utc::now();

    assert!(
        template.created_at() >= &before && template.created_at() <= &after,
        "created_at must be the host's clock at creation: got {}, not between {} and {}",
        template.created_at(),
        before,
        after
    );
    assert_eq!(
        millis(template.created_at()),
        millis(template.updated_at()),
        "a template created now was updated now"
    );
    assert_eq!(template.steps().len(), 1);
}

#[test]
fn a_supplied_timestamp_is_overwritten_by_the_hosts_clock() {
    // 2000-01-01Z: a clock no host running this test can be at, so "the stamp won" is unambiguous.
    let invented = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
    let mut payload = documented_plan();
    payload["created_at"] = serde_json::json!(invented.to_rfc3339());
    payload["updated_at"] = serde_json::json!(invented.to_rfc3339());

    let before = Utc::now();
    let template =
        PlanTemplate::from_create_json(payload).expect("an extra field is not a refusal");
    let after = Utc::now();

    assert_ne!(
        millis(template.created_at()),
        millis(&invented),
        "the caller's clock must not become the host's record"
    );
    assert!(
        template.created_at() >= &before && template.created_at() <= &after,
        "the host's stamp wins: got {}, not between {} and {}",
        template.created_at(),
        before,
        after
    );
    // Overwritten, not refused: the call still succeeded and the payload was otherwise honoured.
    assert_eq!(template.name(), "weekly-audit");
}

#[test]
fn a_stored_document_without_created_at_is_still_refused() {
    // The read door keeps both fields required. `#[serde(default = "Utc::now")]` on the type would
    // make a corrupt stored document indistinguishable from a fresh one, which is exactly the
    // distinction the create door's stamp exists to preserve.
    let err = PlanTemplate::from_json(documented_plan())
        .expect_err("a stored document without its timestamps is corrupt evidence");

    let text = err.to_string();
    assert!(
        text.contains("missing field `created_at`"),
        "the refusal must name the missing field, because that is what an author has to fix: {text}"
    );
}

#[test]
fn the_tool_input_accepts_the_documented_client_shape() {
    // The door a client actually knocks on: `create_template`'s arguments hold the plan. Before the
    // stamp the refusal happened while parsing the arguments, before any handler ran, so a downstream
    // fix could not have reached it.
    let arguments = serde_json::json!({ "name": "weekly-audit", "plan": documented_plan() });

    let before = Utc::now();
    let input: CreateTemplateInput =
        serde_json::from_value(arguments).expect("the documented client shape deserialises");
    let after = Utc::now();

    assert!(
        input.plan.created_at() >= &before && input.plan.created_at() <= &after,
        "the input's plan carries the host's clock: got {}",
        input.plan.created_at()
    );
}
