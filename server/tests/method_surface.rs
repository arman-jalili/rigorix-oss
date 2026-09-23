//! Method-surface coverage for the execution, audit, and template groups
//! (#888 PR C+D).
//!
//! The server forwards every catalog method through one path
//! (`HostBackend::dispatch` → `handle_tool_call`), so parity is by
//! construction. These tests pin the catalog contract — auth level + MCP tool
//! per method — and prove the transport forwards params verbatim and returns
//! the dispatcher's result for every method in each group.

use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{Value, json};

use rigorix_mcp::host::error::HostError;
use rigorix_server::backend::MethodBackend;
use rigorix_server::catalog::{AuthLevel, CATALOG, find};
use rigorix_server::rpc;

/// Records dispatched methods and echoes params.
struct RecordingBackend {
    auth_configured: bool,
    authenticated: bool,
    seen: Mutex<Vec<(String, Value)>>,
}

impl RecordingBackend {
    fn new(auth_configured: bool, authenticated: bool) -> Self {
        Self {
            auth_configured,
            authenticated,
            seen: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl MethodBackend for RecordingBackend {
    async fn dispatch(&self, method: &str, params: Value) -> Result<Value, HostError> {
        self.seen
            .lock()
            .unwrap()
            .push((method.to_string(), params.clone()));
        Ok(json!({ "method": method, "params": params }))
    }
    async fn auth_configured(&self) -> bool {
        self.auth_configured
    }
    async fn is_authenticated(&self) -> bool {
        self.authenticated
    }
}

fn request(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
}

fn assert_group(methods: &[(&str, AuthLevel, &str)]) {
    for (name, auth, tool) in methods {
        let entry = find(name).unwrap_or_else(|| panic!("{name} missing from catalog"));
        assert_eq!(entry.auth, *auth, "{name} auth");
        assert_eq!(entry.mcp_tool, Some(*tool), "{name} mcp tool");
    }
}

#[test]
fn execution_methods_have_expected_auth_and_mcp_tools() {
    assert_group(&[
        ("rigorix.plan", AuthLevel::Public, "rigorix_plan"),
        (
            "rigorix.validatePlan",
            AuthLevel::Public,
            "rigorix_validate_plan",
        ),
        ("rigorix.run", AuthLevel::Session, "rigorix_run"),
        ("rigorix.execute", AuthLevel::Session, "rigorix_execute"),
        (
            "rigorix.approve",
            AuthLevel::Session,
            "rigorix_approve_execution",
        ),
        (
            "rigorix.checkEnforcement",
            AuthLevel::Public,
            "rigorix_check_enforcement",
        ),
    ]);
}

#[test]
fn audit_methods_have_expected_auth_and_mcp_tools() {
    assert_group(&[
        (
            "rigorix.audit.read",
            AuthLevel::Session,
            "rigorix_read_audit",
        ),
        (
            "rigorix.audit.list",
            AuthLevel::Session,
            "rigorix_list_audits",
        ),
        (
            "rigorix.audit.summary",
            AuthLevel::Session,
            "rigorix_audit_summary",
        ),
    ]);
}

#[test]
fn template_methods_have_expected_auth_and_mcp_tools() {
    assert_group(&[
        (
            "rigorix.template.list",
            AuthLevel::Session,
            "rigorix_list_templates",
        ),
        (
            "rigorix.template.get",
            AuthLevel::Session,
            "rigorix_get_template",
        ),
        (
            "rigorix.template.create",
            AuthLevel::Session,
            "rigorix_create_template",
        ),
        (
            "rigorix.template.validate",
            AuthLevel::Public,
            "rigorix_validate_template",
        ),
    ]);
}

#[tokio::test]
async fn every_public_mcp_method_forwards_params_and_returns_result() {
    // Covers execution (plan/validatePlan/checkEnforcement), audit
    // (none public), template.validate, auth.*, usageGuide.
    let backend = RecordingBackend::new(false, false);
    let params = json!({ "probe": 7, "nested": { "a": [1, 2] } });

    for entry in CATALOG {
        if entry.auth != AuthLevel::Public || entry.mcp_tool.is_none() {
            continue;
        }
        // `system.version` is handled by the server itself.
        if entry.name == "rigorix.system.version" {
            continue;
        }
        let response = rpc::handle_value(request(entry.name, params.clone()), &backend)
            .await
            .expect("responds");
        assert_eq!(
            response["result"]["method"], entry.name,
            "{} must dispatch",
            entry.name
        );
        assert_eq!(
            response["result"]["params"], params,
            "{} must forward params verbatim",
            entry.name
        );
    }

    // Params reached the backend once per public method.
    let seen = backend.seen.lock().unwrap();
    assert!(!seen.is_empty());
    for (_, forwarded) in seen.iter() {
        assert_eq!(*forwarded, params);
    }
}

#[tokio::test]
async fn every_session_mcp_method_is_refused_unattested_then_dispatches_when_attested() {
    let params = json!({ "probe": true });

    let unattested = RecordingBackend::new(true, false);
    for entry in CATALOG {
        if entry.auth != AuthLevel::Session {
            continue;
        }
        let response = rpc::handle_value(request(entry.name, params.clone()), &unattested)
            .await
            .expect("responds");
        assert_eq!(
            response["error"]["code"], -32001,
            "{} must require a session",
            entry.name
        );
    }

    let attested = RecordingBackend::new(true, true);
    for entry in CATALOG {
        if entry.auth != AuthLevel::Session {
            continue;
        }
        let response = rpc::handle_value(request(entry.name, params.clone()), &attested)
            .await
            .expect("responds");
        assert_eq!(
            response["result"]["method"], entry.name,
            "{} must dispatch when attested",
            entry.name
        );
    }
}
