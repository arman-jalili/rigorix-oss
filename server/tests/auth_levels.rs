//! Auth-level enforcement + identity invariant (#888 OSS-C2, ADR-0001 D4/D5).
//!
//! The server enforces public/session/admin per catalog entry. With an IdP
//! configured, an unattested session method is refused; identity is taken from
//! the attested session, never from params (the composed host injects it).

use async_trait::async_trait;
use serde_json::{Value, json};

use rigorix_mcp::host::error::HostError;
use rigorix_server::backend::MethodBackend;
use rigorix_server::rpc;

#[derive(Clone, Copy)]
struct AuthStub {
    auth_configured: bool,
    authenticated: bool,
}

#[async_trait]
impl MethodBackend for AuthStub {
    async fn dispatch(&self, method: &str, _params: Value) -> Result<Value, HostError> {
        Ok(json!({ "dispatched": method }))
    }
    async fn auth_configured(&self) -> bool {
        self.auth_configured
    }
    async fn is_authenticated(&self) -> bool {
        self.authenticated
    }
}

fn request(id: i64, method: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": {} })
}

async fn call(backend: &AuthStub, method: &str) -> Value {
    rpc::handle_value(request(1, method), backend)
        .await
        .expect("responds")
}

#[tokio::test]
async fn public_method_is_allowed_unauthenticated() {
    let backend = AuthStub {
        auth_configured: true,
        authenticated: false,
    };
    let response = call(&backend, "rigorix.plan").await;
    assert_eq!(response["result"]["dispatched"], "rigorix.plan");
}

#[tokio::test]
async fn session_method_is_refused_when_idp_configured_and_unattested() {
    let backend = AuthStub {
        auth_configured: true,
        authenticated: false,
    };
    let response = call(&backend, "rigorix.run").await;
    assert_eq!(response["error"]["code"], -32001);
    assert_eq!(response["error"]["data"]["type"], "not_authenticated");
}

#[tokio::test]
async fn session_method_is_allowed_when_attested() {
    let backend = AuthStub {
        auth_configured: true,
        authenticated: true,
    };
    let response = call(&backend, "rigorix.run").await;
    assert_eq!(response["result"]["dispatched"], "rigorix.run");
}

#[tokio::test]
async fn session_method_allowed_in_local_mode_without_idp() {
    // Local/legacy mode (no IdP) does not refuse — matches the stdio host.
    let backend = AuthStub {
        auth_configured: false,
        authenticated: false,
    };
    let response = call(&backend, "rigorix.execute").await;
    assert_eq!(response["result"]["dispatched"], "rigorix.execute");
}

#[tokio::test]
async fn admin_method_is_not_enabled_in_oss() {
    let backend = AuthStub {
        auth_configured: true,
        authenticated: true,
    };
    let response = call(&backend, "rigorix.policy.bundle").await;
    assert_eq!(response["error"]["code"], -32031);
    assert_eq!(response["error"]["data"]["type"], "not_enabled");
}
