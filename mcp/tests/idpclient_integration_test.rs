//! Integration tests for HttpIdpClient (Infrastructure) — ISSUE-AUTH-2.
//!
//! Exercises the REAL reqwest-backed client against a local loopback mock
//! OIDC IdP (RFC 8414 discovery + RFC 8628 device flow + RFC 6749 refresh +
//! RFC 7009 revocation). Plain HTTP is accepted only for loopback addresses
//! (test policy) — production issuers stay HTTPS-only.
//!
//! @canonical .pi/architecture/modules/auth.md#idpclient-infrastructure
//! Implements: ISSUE-AUTH-2 — full RFC round-trip through HttpIdpClient

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rigorix_mcp::auth::infrastructure::{HttpIdpClient, IdpClient, TokenPoll};

// ---------------------------------------------------------------------------
// Minimal loopback mock OIDC IdP (raw HTTP/1.1 over TCP)
// ---------------------------------------------------------------------------

struct MockIdp {
    addr: String,
    token_polls: Arc<AtomicUsize>,
    refresh_calls: Arc<AtomicUsize>,
    revocations: Arc<AtomicUsize>,
    deny_tokens: bool,
}

impl MockIdp {
    async fn spawn() -> Self {
        Self::spawn_with_mode(false).await
    }

    /// A mock whose token endpoint always answers `access_denied`.
    async fn spawn_denying() -> Self {
        Self::spawn_with_mode(true).await
    }

    async fn spawn_with_mode(deny_tokens: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let token_polls = Arc::new(AtomicUsize::new(0));
        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let revocations = Arc::new(AtomicUsize::new(0));

        {
            let token_polls = token_polls.clone();
            let refresh_calls = refresh_calls.clone();
            let revocations = revocations.clone();
            tokio::spawn(async move {
                loop {
                    let (mut socket, _) = match listener.accept().await {
                        Ok(pair) => pair,
                        Err(_) => break,
                    };
                    let token_polls = token_polls.clone();
                    let refresh_calls = refresh_calls.clone();
                    let revocations = revocations.clone();
                    tokio::spawn(async move {
                        use tokio::io::{AsyncReadExt, AsyncWriteExt};

                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            async {
                                let mut buf = Vec::new();
                                let mut tmp = [0u8; 1024];
                                // Read until the header terminator.
                                let mut header_end = None;
                                while header_end.is_none() {
                                    let n = socket.read(&mut tmp).await?;
                                    if n == 0 {
                                        return Ok::<(), std::io::Error>(());
                                    }
                                    buf.extend_from_slice(&tmp[..n]);
                                    header_end = find_subsequence(&buf, b"\r\n\r\n");
                                }
                                let header_end = header_end.unwrap();
                                let head =
                                    String::from_utf8_lossy(&buf[..header_end]).to_string();
                                let body_start = header_end + 4;

                                // Content-Length body (form posts).
                                let mut length = 0usize;
                                for line in head.lines() {
                                    let lower = line.to_ascii_lowercase();
                                    if let Some(rest) = lower.strip_prefix("content-length:") {
                                        length = rest.trim().parse().unwrap_or(0);
                                    }
                                }
                                while buf.len() < body_start + length {
                                    let n = socket.read(&mut tmp).await?;
                                    if n == 0 {
                                        break;
                                    }
                                    buf.extend_from_slice(&tmp[..n]);
                                }
                                let body =
                                    String::from_utf8_lossy(&buf[body_start..body_start + length])
                                        .to_string();

                                let request_line = head.lines().next().unwrap_or("").to_string();
                                let path = request_line
                                    .split_whitespace()
                                    .nth(1)
                                    .unwrap_or("/")
                                    .to_string();
                                let base = format!("http://{addr}");

                                let (status, json) = route(
                                    &base,
                                    &path,
                                    &body,
                                    &token_polls,
                                    &refresh_calls,
                                    &revocations,
                                    deny_tokens,
                                );

                                let resp_head = format!(
                                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    json.len()
                                );
                                socket.write_all(resp_head.as_bytes()).await?;
                                socket.write_all(json.as_bytes()).await?;
                                socket.flush().await?;
                                Ok::<(), std::io::Error>(())
                            },
                        )
                        .await;
                    });
                }
            });
        }

        Self {
            addr: format!("http://{addr}"),
            token_polls,
            refresh_calls,
            revocations,
            deny_tokens,
        }
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Minimal percent-decoding (application/x-www-form-urlencoded bodies).
fn decode_form(body: &str) -> String {
    let bytes = body.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn route(
    base: &str,
    path: &str,
    body: &str,
    token_polls: &AtomicUsize,
    refresh_calls: &AtomicUsize,
    revocations: &AtomicUsize,
    deny_tokens: bool,
) -> (&'static str, String) {
    match path {
        "/.well-known/openid-configuration" => (
            "200 OK",
            format!(
                r#"{{
                    "issuer": "{base}",
                    "device_authorization_endpoint": "{base}/device_authorization",
                    "token_endpoint": "{base}/token",
                    "revocation_endpoint": "{base}/revoke",
                    "jwks_uri": "{base}/jwks"
                }}"#
            ),
        ),
        "/device_authorization" => (
            "200 OK",
            r#"{
                "device_code": "dc-integration-1",
                "user_code": "ABCD-EFGH",
                "verification_uri": "https://idp.example.com/device",
                "expires_in": 600,
                "interval": 5
            }"#
            .to_string(),
        ),
        "/token" => {
            let decoded = decode_form(body);
            if deny_tokens {
                return (
                    "400 Bad Request",
                    r#"{"error":"access_denied","error_description":"user declined"}"#.to_string(),
                );
            }
            if decoded.contains("urn:ietf:params:oauth:grant-type:device_code") {
                // First poll: authorization_pending; second: success.
                let n = token_polls.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    (
                        "400 Bad Request",
                        r#"{"error":"authorization_pending"}"#.to_string(),
                    )
                } else {
                    (
                        "200 OK",
                        r#"{
                            "access_token": "at-integration",
                            "refresh_token": "rt-integration",
                            "expires_in": 900,
                            "token_type": "Bearer",
                            "scope": "openid offline_access"
                        }"#
                        .to_string(),
                    )
                }
            } else if decoded.contains("grant_type=refresh_token") {
                refresh_calls.fetch_add(1, Ordering::SeqCst);
                (
                    "200 OK",
                    r#"{
                        "access_token": "at-refreshed",
                        "expires_in": 900,
                        "token_type": "Bearer"
                    }"#
                    .to_string(),
                )
            } else {
                (
                    "400 Bad Request",
                    r#"{"error":"unsupported_grant_type"}"#.to_string(),
                )
            }
        }
        "/revoke" => {
            revocations.fetch_add(1, Ordering::SeqCst);
            ("200 OK", "{}".to_string())
        }
        _ => ("404 Not Found", r#"{"error":"not_found"}"#.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Full RFC round trip: discovery → device authorization → pending poll →
/// successful poll → refresh → revoke.
#[tokio::test]
async fn http_client_completes_rfc_device_flow_round_trip() {
    let mock = MockIdp::spawn().await;
    let client = HttpIdpClient::new(&mock.addr).expect("loopback issuer accepted");

    // RFC 8414 — discovery (cached).
    let meta = client.discover().await.unwrap();
    assert_eq!(meta.issuer, mock.addr);
    assert!(meta.revocation_endpoint.is_some());
    // Cached on the second call.
    let meta2 = client.discover().await.unwrap();
    assert_eq!(meta2.issuer, mock.addr);

    // RFC 8628 §3.1 — device authorization.
    let auth = client.device_authorization("rigorix-cli").await.unwrap();
    assert_eq!(auth.device_code.expose(), "dc-integration-1");
    assert_eq!(auth.user_code, "ABCD-EFGH");
    assert_eq!(auth.expires_in, 600);

    // RFC 8628 §3.5 — first poll pending…
    let poll1 = client
        .poll_token(&auth.device_code, "rigorix-cli")
        .await
        .unwrap();
    assert!(matches!(poll1, TokenPoll::Pending { .. }));

    // …second poll succeeds with tokens.
    let poll2 = client
        .poll_token(&auth.device_code, "rigorix-cli")
        .await
        .unwrap();
    let refresh_secret = match poll2 {
        TokenPoll::Succeeded(resp) => {
            assert_eq!(resp.access_token.expose(), "at-integration");
            let refresh = resp.refresh_token.clone().expect("refresh token present");
            assert_eq!(refresh.expose(), "rt-integration");
            refresh
        }
        other => panic!("expected token success, got {other:?}"),
    };

    // RFC 6749 §6 — refresh with the refresh token.
    let refreshed = client
        .refresh_token(&refresh_secret, "rigorix-cli")
        .await
        .unwrap();
    assert_eq!(refreshed.access_token.expose(), "at-refreshed");
    assert_eq!(mock.refresh_calls.load(Ordering::SeqCst), 1);

    // RFC 7009 — revocation.
    client
        .revoke_token(&refresh_secret, "rigorix-cli")
        .await
        .unwrap();
    assert_eq!(mock.revocations.load(Ordering::SeqCst), 1);

    // The token endpoint was polled exactly twice (pending + success).
    assert_eq!(mock.token_polls.load(Ordering::SeqCst), 2);
}

/// RFC 8628 denial surfaces as a typed AccessDenied outcome.
#[tokio::test]
async fn http_client_poll_denial_is_typed() {
    let mock = MockIdp::spawn_denying().await;
    assert!(mock.deny_tokens);
    let client = HttpIdpClient::new(&mock.addr).unwrap();
    let _meta = client.discover().await.unwrap();
    let auth = client.device_authorization("rigorix-cli").await.unwrap();
    let outcome = client
        .poll_token(&auth.device_code, "rigorix-cli")
        .await
        .unwrap();
    match outcome {
        TokenPoll::AccessDenied { reason } => assert_eq!(reason, "user declined"),
        other => panic!("expected denial, got {other:?}"),
    }
}
