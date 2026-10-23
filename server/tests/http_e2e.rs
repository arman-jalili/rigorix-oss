//! End-to-end HTTP/SSE transport (#1209).
//!
//! Binds an ephemeral loopback port and drives the **production** route table
//! with a real HTTP client. The other server suites stop at the codec
//! (`rpc_transport`), the hub (`events_live`), or `oneshot` without a socket
//! (`mcp_streamable_http`); none proves a client can consume the status line,
//! `text/event-stream`, chunked frames, or a `Last-Event-ID` reconnect.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::Router;
use rigorix_mcp::host::error::HostError;
use rigorix_server::app;
use rigorix_server::backend::MethodBackend;
use rigorix_server::events::{EventHub, EventKind};
use rigorix_server::state::ServerState;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::time::timeout;

/// Echoes the method + params so the test can prove the request crossed the
/// socket, not just the router.
struct EchoBackend;

#[async_trait]
impl MethodBackend for EchoBackend {
    async fn dispatch(&self, method: &str, params: Value) -> Result<Value, HostError> {
        Ok(json!({ "method": method, "params": params }))
    }
}

async fn serve() -> (String, Arc<EventHub>, tokio::task::JoinHandle<()>) {
    let hub = Arc::new(EventHub::new());
    let state = ServerState::new(Arc::new(EchoBackend), Arc::clone(&hub));
    let router: Router = app::router(state);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind an ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serve");
    });
    (format!("http://{addr}"), hub, handle)
}

/// Read one complete SSE frame (`\n\n`-terminated), ignoring keep-alive
/// comments. The timeout keeps a framing regression from hanging CI.
async fn next_frame(response: &mut reqwest::Response) -> String {
    let mut buffer = String::new();
    loop {
        let chunk = timeout(Duration::from_secs(5), response.chunk())
            .await
            .expect("SSE frame within 5s")
            .expect("read the event stream")
            .expect("a frame chunk");
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(end) = buffer.find("\n\n") {
            let frame = buffer[..end].to_string();
            buffer.drain(..end + 2);
            if frame
                .lines()
                .any(|line| !line.starts_with(':') && !line.is_empty())
            {
                return frame;
            }
        }
    }
}

fn field<'a>(frame: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}: ");
    frame.lines().find_map(|line| line.strip_prefix(&prefix))
}

#[tokio::test]
async fn a_real_client_reads_json_rpc_and_sse_over_a_socket() {
    let (base, hub, server) = serve().await;
    let client = reqwest::Client::new();

    // 1. JSON-RPC over the wire: status line, JSON content type, result body.
    let response = client
        .post(format!("{base}/rpc"))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "rigorix.plan",
            "params": {"intent": "ship it"}
        }))
        .send()
        .await
        .expect("POST /rpc");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json")),
        "JSON-RPC response must be JSON"
    );
    let body: Value = response.json().await.expect("JSON body");
    assert_eq!(body["id"], 7);
    assert_eq!(body["result"]["method"], "rigorix.plan");
    assert_eq!(body["result"]["params"]["intent"], "ship it");

    // 2. SSE over the wire: the response is a real event stream, and an event
    // published after the client connected arrives as a framed SSE event.
    let mut live = client
        .get(format!("{base}/events"))
        .send()
        .await
        .expect("GET /events");
    assert_eq!(live.status(), reqwest::StatusCode::OK);
    assert_eq!(
        live.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream"),
        "SSE must be declared as text/event-stream"
    );

    let first = hub.publish(
        EventKind::RunProgress,
        json!({"event_type": "node_completed", "marker": "first"}),
    );
    let frame = next_frame(&mut live).await;
    assert_eq!(field(&frame, "event"), Some("run_progress"));
    assert_eq!(field(&frame, "id"), Some(first.to_string().as_str()));
    assert!(
        field(&frame, "data").is_some_and(|data| data.contains("\"marker\":\"first\"")),
        "data payload must reach the client: {frame}"
    );
    drop(live);

    // 3. Reconnect with Last-Event-ID: publish while disconnected, then prove
    // the replay starts after the id the client already saw.
    let second = hub.publish(
        EventKind::PolicyChanged,
        json!({"event_type": "policy_changed", "marker": "second"}),
    );
    let mut replay = client
        .get(format!("{base}/events"))
        .header("Last-Event-ID", first.to_string())
        .send()
        .await
        .expect("GET /events with Last-Event-ID");
    assert_eq!(replay.status(), reqwest::StatusCode::OK);

    let frame = next_frame(&mut replay).await;
    assert_eq!(field(&frame, "event"), Some("policy_changed"));
    assert_eq!(field(&frame, "id"), Some(second.to_string().as_str()));
    assert!(
        field(&frame, "data").is_some_and(|data| data.contains("\"marker\":\"second\"")),
        "replay must deliver the missed event: {frame}"
    );

    server.abort();
}
