//! #1656 — a plan constraint is enforced at the one door a plan is parsed through, never accepted
//! and ignored.
//!
//! `constraints.blocked_tools` and `constraints.max_tool_calls` appeared in the schema, were echoed
//! back to the caller, and had **zero** effect: `blocked_tools: ["run_command"]` ran `run_command`
//! and both steps succeeded. Both `rigorix_execute` and `rigorix_validate_plan` parse through
//! `PlanTemplate::from_json`, so that is where the refusal lives and both routes get it.

use rigorix_mcp::execution_tools::domain::value::{PlanTemplate, PlanTemplateError};
use serde_json::json;

/// The deserializer requires `blocked_tools` and `extensions` even though the advertised schema does
/// not (#1657 records that); a test that builds a document the deserializer refuses would be testing
/// the wrong thing.
fn constraints(value: serde_json::Value) -> serde_json::Value {
    let mut object = value.as_object().cloned().unwrap_or_default();
    object.entry("blocked_tools").or_insert_with(|| json!([]));
    object.entry("extensions").or_insert_with(|| json!({}));
    serde_json::Value::Object(object)
}

fn plan(constraints_in: serde_json::Value, tools: &[&str]) -> serde_json::Value {
    json!({
        "name": "constrained",
        "description": "a plan",
        "steps": tools
            .iter()
            .enumerate()
            .map(|(index, tool)| json!({
                "name": format!("step{index}"),
                "tool": tool,
                "parameters": {},
                "requires_approval": false,
                "description": "a step",
            }))
            .collect::<Vec<_>>(),
        "constraints": constraints(constraints_in),
    })
}

#[test]
fn a_blocked_tool_the_plan_uses_is_refused_before_dispatch() {
    let error = PlanTemplate::from_json(plan(
        json!({ "blocked_tools": ["run_command"] }),
        &["file_read", "run_command"],
    ))
    .expect_err("the plan uses a tool it blocks");

    match error {
        PlanTemplateError::BlockedTool { tool, step } => {
            assert_eq!(tool, "run_command");
            assert_eq!(step, "step1", "the refusal names the step that uses it");
        }
        other => panic!("expected BlockedTool, got {other:?}"),
    }
    // The message is what an agent reads; it must say the constraint is enforced, not recorded.
    let message = PlanTemplateError::BlockedTool {
        tool: "run_command".to_string(),
        step: "step1".to_string(),
    }
    .to_string();
    assert!(message.contains("before dispatch"), "{message}");
}

#[test]
fn a_blocked_tool_the_plan_does_not_use_is_accepted() {
    PlanTemplate::from_json(plan(
        json!({ "blocked_tools": ["run_command"] }),
        &["file_read", "file_write"],
    ))
    .expect("a constraint that the plan satisfies is not a failure");
}

#[test]
fn a_call_budget_below_the_step_count_is_refused() {
    let error = PlanTemplate::from_json(plan(
        json!({ "max_tool_calls": 1 }),
        &["file_read", "file_write"],
    ))
    .expect_err("two steps cannot fit a budget of one");
    match error {
        PlanTemplateError::TooManyToolCalls { declared, calls } => {
            assert_eq!(declared, 1);
            assert_eq!(calls, 2);
        }
        other => panic!("expected TooManyToolCalls, got {other:?}"),
    }
}

#[test]
fn a_call_budget_at_or_above_the_step_count_is_accepted() {
    PlanTemplate::from_json(plan(
        json!({ "max_tool_calls": 2 }),
        &["file_read", "file_write"],
    ))
    .expect("an exact budget fits");
    PlanTemplate::from_json(plan(json!({ "max_tool_calls": 9 }), &["file_read"]))
        .expect("a generous budget fits");
}

#[test]
fn no_constraints_means_no_refusal() {
    PlanTemplate::from_json(json!({
        "name": "plain",
        "description": "a plan",
        "steps": [{
            "name": "s",
            "tool": "run_command",
            "parameters": {"command": "true"},
            "requires_approval": false,
            "description": "a step"
        }],
    }))
    .expect("an unconstrained plan is not refused");
}
