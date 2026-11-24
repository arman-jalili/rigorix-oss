//! #1657 — the advertised schema is the contract, validation sees what a reader needs, and the
//! caller gets the output.

use rigorix_mcp::execution_tools::domain::value::PlanTemplate;
use rigorix_mcp::execution_tools::interfaces::mcp::RIGORIX_EXECUTE_INPUT_SCHEMA;
use serde_json::json;

/// A document built **only** from the fields the advertised schema requires.
///
/// The audit's case: a schema-conformant client got four sequential `missing field` errors, one per
/// round trip, because the deserializer demanded `plan.description`, `step.description`,
/// `step.requires_approval`, `constraints.blocked_tools` and `constraints.extensions`.
#[test]
fn a_document_built_from_the_advertised_schema_is_accepted_on_the_first_call() {
    let schema: serde_json::Value =
        serde_json::from_str(RIGORIX_EXECUTE_INPUT_SCHEMA).expect("the advertised schema parses");
    let plan_required = schema["properties"]["plan"]["required"]
        .as_array()
        .expect("plan.required");
    let step_required = schema["properties"]["plan"]["properties"]["steps"]["items"]["required"]
        .as_array()
        .expect("step.required");

    // Build the plan from the schema's own `required` lists, with one value per declared type.
    let mut plan = serde_json::Map::new();
    for field in plan_required {
        let name = field.as_str().expect("a required name");
        plan.insert(
            name.to_string(),
            match name {
                "steps" => json!([step_document(step_required)]),
                _ => json!("value"),
            },
        );
    }
    let document = json!({ "plan": serde_json::Value::Object(plan) });

    PlanTemplate::from_json(document["plan"].clone())
        .expect("a document built from the advertised required fields must parse");
}

fn step_document(required: &[serde_json::Value]) -> serde_json::Value {
    let mut step = serde_json::Map::new();
    for field in required {
        let name = field.as_str().expect("a required name");
        step.insert(
            name.to_string(),
            match name {
                "tool" => json!("file_read"),
                "parameters" => json!({ "path": "x" }),
                _ => json!("step"),
            },
        );
    }
    serde_json::Value::Object(step)
}

/// The control: the schema must not have quietly grown the fields it was missing — the fix is that
/// the *deserializer* agrees with the schema, not that the schema moved to match the deserializer.
#[test]
fn the_advertised_required_sets_are_unchanged() {
    let schema: serde_json::Value =
        serde_json::from_str(RIGORIX_EXECUTE_INPUT_SCHEMA).expect("schema");
    assert_eq!(
        schema["properties"]["plan"]["required"],
        json!(["name", "description", "steps"])
    );
    assert_eq!(
        schema["properties"]["plan"]["properties"]["steps"]["items"]["required"],
        json!(["name", "tool", "parameters"])
    );
}

/// The dispatcher's tool list is the one a validator asks (#1657): it must be non-empty and contain
/// the tools the conversion produces.
#[test]
fn the_engine_publishes_the_tool_names_a_validator_needs() {
    use rigorix_engine::execution_engine::application::service_impl::DISPATCHABLE_TOOL_NAMES;
    assert!(
        DISPATCHABLE_TOOL_NAMES.len() >= 9,
        "{DISPATCHABLE_TOOL_NAMES:?}"
    );
    for expected in [
        "run_command",
        "file_read",
        "file_write",
        "file_patch",
        "git_commit",
    ] {
        assert!(
            DISPATCHABLE_TOOL_NAMES.contains(&expected),
            "`{expected}` is missing from the published list"
        );
    }
    // The list must not contain a name the plan conversion cannot produce, or a validator would
    // accept a step the adapter never emits.
    assert!(!DISPATCHABLE_TOOL_NAMES.contains(&"definitely_not_a_tool"));
}
