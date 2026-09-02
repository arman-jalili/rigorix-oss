//! Tool-descriptor schema validation (GAP-L-08).
//!
//! Every `rigorix_*_tool_descriptor()` embeds an `inputSchema` parsed from a
//! compile-time `*_INPUT_SCHEMA` string constant. Those parses use
//! `expect("schema const is valid JSON (test-enforced)")` — this test is the
//! enforcement: if any const is edited into invalid JSON, this suite fails at
//! build/test time instead of at runtime.

#[test]
fn all_oss_tool_descriptor_schemas_are_valid_json() {
    let descriptors: Vec<serde_json::Value> = vec![
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_execute_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_validate_plan_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_check_enforcement_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_approve_execution_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_plan_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_run_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_list_templates_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_get_template_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_create_template_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_validate_template_tool_descriptor(),
        rigorix_mcp::audit_tools::interfaces::mcp::rigorix_read_audit_tool_descriptor(),
        rigorix_mcp::audit_tools::interfaces::mcp::rigorix_list_audits_tool_descriptor(),
        rigorix_mcp::audit_tools::interfaces::mcp::rigorix_audit_summary_tool_descriptor(),
        rigorix_mcp::usage_guide::interfaces::mcp::rigorix_get_usage_guide_tool_descriptor(),
    ];

    assert_eq!(descriptors.len(), 14, "all OSS tool descriptors enumerated");
    for d in descriptors {
        let name = d
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown-tool");
        let schema = d
            .get("inputSchema")
            .unwrap_or_else(|| panic!("{name}: descriptor missing inputSchema"));
        assert!(
            schema.is_object(),
            "{name}: inputSchema must be a JSON object"
        );
    }
}

#[test]
fn enterprise_descriptors_are_valid_json() {
    let descriptors = vec![
        rigorix_mcp::enterprise_proxy::interfaces::mcp::rigorix_enterprise_call_tool_descriptor(),
        rigorix_mcp::enterprise_proxy::interfaces::mcp::rigorix_enterprise_health_tool_descriptor(),
    ];
    assert_eq!(
        descriptors.len(),
        2,
        "all enterprise tool descriptors enumerated"
    );
    for d in descriptors {
        assert!(
            d.get("inputSchema")
                .unwrap_or(&serde_json::Value::Null)
                .is_object()
        );
    }
}
