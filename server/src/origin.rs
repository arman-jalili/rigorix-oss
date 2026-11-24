//! The HTTP front door's **one** origin decision (#1650).
//!
//! @canonical .pi/architecture/modules/rigorix-server.md#http-surface
//!
//! # Why this module exists
//!
//! The origin guard existed exactly once, in `mcp_http.rs`, and was wired to `/mcp` only. `/rpc`
//! had none, so a page served from any origin on the internet could drive a free-mode executor on
//! `127.0.0.1:3001` — and `/rpc` did not require a JSON content type either, so a browser could send
//! it as a **simple request** that never preflights. `/events` had none. The audit's sentence is the
//! exact shape of the defect: *a control implemented on one of several routes*.
//!
//! The fix is not three guards. It is one decision, called by all three routes, with the reasoning
//! written down once. A second copy is how the third route gets forgotten.
//!
//! # The rule, stated once
//!
//! A browser attaches `Origin` to a cross-origin request — POST, form submission, `fetch`,
//! `sendBeacon`, image, everything. A non-browser client attaches nothing. So:
//!
//! | request | decision |
//! |---|---|
//! | no `Origin` | **allowed** — a non-browser client; it is not the attacker this defends against |
//! | `Origin: null` | **refused** — a sandboxed iframe, a `data:` document or a redirect; "null" is not "no origin" |
//! | origin host is loopback (`localhost`, `127.0.0.1`, `::1`) | allowed — the shipping deployment |
//! | origin host equals the request's `Host` | allowed — a same-origin page |
//! | anything else | **refused**, and the refusal names the origin from the **Origin** header |
//!
//! and, for a request that **carries a body**, a second clause that closes the preflight-free path:
//!
//! | request | decision |
//! |---|---|
//! | `Origin` present, content type is not `application/json` | **refused** — this is the "simple request" a browser can send without asking |
//! | `Origin` absent | unchanged — a client that sends no origin is not restricted to JSON |
//!
//! # What this does not do
//!
//! It does not authenticate anyone. Free mode (no IdP) is a **supported** choice: `AuthLevel::Session`
//! methods are open, `/events` is unauthenticated, and identity comes from the caller. That is the
//! documented quickstart and it stays — it is stated as a non-claim in `docs/deployment.md` and
//! warned about, loudly, when the bind is not loopback.
//!
//! It does not defend against a **local** attacker who can send a request with no `Origin` at all:
//! that is a token/binding problem (separate work item), not an origin problem. It also does not
//! stop DNS rebinding for a request whose `Host` the attacker controls — the loopback-bind default is
//! what keeps that unreachable, which is why the non-loopback bind warns.

use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// The outcome of the one origin decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginDecision {
    /// The request may proceed.
    Allowed,
    /// The request is refused; `reason` is written for the operator the page is attacking.
    Refused { reason: String },
}

/// Decide, for one request, whether it may reach a handler.
///
/// `body_carrying` is about the route, not the request: `/rpc` and `/mcp` accept a body and therefore
/// admit the simple-request shape; `/events` does not, so the content-type clause does not apply to it.
pub fn decide(headers: &HeaderMap, body_carrying: bool) -> OriginDecision {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());

    let Some(origin) = origin else {
        // No Origin: a non-browser client (curl, an SDK, the Action). Not the attacker here.
        return OriginDecision::Allowed;
    };

    if origin == "null" {
        return OriginDecision::Refused {
            reason: format!(
                "the request's Origin is `null`, which is what a sandboxed iframe, a `data:` \
                 document or a redirect sends — it is not the same as sending no Origin, and it is \
                 not a same-origin page. Refused on every route ({ROUTE_SUBJECT})"
            ),
        };
    }

    if let Some(reason) = origin_host_refusal(origin, headers) {
        return OriginDecision::Refused { reason };
    }

    if body_carrying && !is_json_content_type(headers) {
        let declared = headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("(none)");
        return OriginDecision::Refused {
            reason: format!(
                "the request carries an Origin (so a browser sent it) and its content type is \
                 `{declared}`, not `application/json`. A browser can send that shape cross-origin \
                 **without a preflight**, so answering it would let a page on any origin drive this \
                 executor; the preflighted JSON shape is the only one this route accepts from a \
                 browser ({ROUTE_SUBJECT})"
            ),
        };
    }

    OriginDecision::Allowed
}

/// The subject of the decision, named in every refusal so a reader knows which rule refused them.
const ROUTE_SUBJECT: &str = "one decision, shared by /rpc, /mcp and /events (#1650)";

fn origin_host_refusal(origin: &str, headers: &HeaderMap) -> Option<String> {
    let origin_host = origin
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .and_then(|authority| authority.rsplit('@').next())
        .map(strip_port)
        .unwrap_or("");
    let host_header = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(strip_port);

    if origin_host.is_empty() {
        return Some(format!(
            "the Origin header `{origin}` is not a URL this server can read a host from, so the \
             origin cannot be shown to be loopback or same-origin. Refused ({ROUTE_SUBJECT})"
        ));
    }
    if is_loopback(origin_host) || host_header == Some(origin_host) {
        return None;
    }
    Some(format!(
        "cross-origin request refused: the page's origin is `{origin_host}` and this server's Host \
         is `{}`. Only a loopback origin or a same-origin page may drive a session on this port — a \
         page you merely have open must not be able to execute anything here ({ROUTE_SUBJECT})",
        host_header.unwrap_or("(absent)")
    ))
}

fn is_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|value| {
            let essence = value.split(';').next().unwrap_or("").trim();
            essence.eq_ignore_ascii_case("application/json")
        })
        .unwrap_or(false)
}

fn strip_port(value: &str) -> &str {
    if let Some(rest) = value.strip_prefix('[') {
        // IPv6 literal: [::1]:3001
        return rest.split(']').next().unwrap_or(rest);
    }
    value.split(':').next().unwrap_or(value)
}

pub fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

/// The response for a refused request, so every route answers with the same shape.
pub fn refusal(decision: OriginDecision, route: &'static str) -> Option<Response> {
    match decision {
        OriginDecision::Allowed => None,
        OriginDecision::Refused { reason } => Some(
            (
                StatusCode::FORBIDDEN,
                axum::Json(serde_json::json!({
                    "error": "origin_refused",
                    "route": route,
                    "message": reason,
                })),
            )
                .into_response(),
        ),
    }
}

/// `OPTIONS` on a body-carrying route.
///
/// A preflight is a browser asking *before* it sends the real request. The answer here is an explicit
/// refusal with **no** CORS grant: a 405, which is what this route used to answer, reads as "you asked
/// the wrong shape" and leaves the simple-request path — the one that needs no preflight — open, which
/// is exactly the hole this module closes.
pub fn preflight_refusal(route: &'static str) -> Response {
    (
        StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({
            "error": "cors_not_granted",
            "route": route,
            "message": format!(
                "this server does not grant cross-origin access to {route}: a browser preflight is \
                 refused, deliberately, and no `Access-Control-*` header is returned. Call this \
                 route from the same origin, from a loopback page, or from a non-browser client \
                 (which needs no preflight) — permissive CORS is what turns an absent preflight \
                 into a readable response (#1650)"
            ),
        })),
    )
        .into_response()
}

/// One loud line when free mode is bound beyond loopback, or `None`.
///
/// Free mode is not refused — it is the documented quickstart. But binding it to a non-loopback
/// address changes what is reachable, and the operator is the only person who can decide that. The
/// line names what becomes reachable rather than saying "insecure".
pub fn free_mode_warning(bind: &str, auth_configured: bool) -> Option<String> {
    if auth_configured {
        return None;
    }
    // Parse the address properly. Splitting on the last `:` turns `0:0:0:0:0:0:0:1:3001` into
    // `0:0:0:0:0:0:0`, which is not a loopback host — a warning for the most loopback address there
    // is. The test that caught this was written before the fix, and it is why this is a match and
    // not a one-line split.
    let host = if let Ok(address) = bind.parse::<std::net::SocketAddr>() {
        address.ip().to_string()
    } else if bind.parse::<std::net::IpAddr>().is_ok() {
        // A bare address with no port (`::1`, `127.0.0.1`) is a bind the CLI accepts.
        bind.to_string()
    } else {
        bind.rsplit_once(':')
            .map(|(host, _)| host)
            .unwrap_or(bind)
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string()
    };
    let loopback = is_loopback(&host)
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if loopback {
        return None;
    }
    Some(format!(
        "⚠ FREE MODE ON A NON-LOOPBACK BIND ({bind}): no IdP is configured, so `rigorix.execute`, \
         `rigorix.approve` and the rest of the Session-authorization surface answer any caller that \
         reaches this port, and GET /events streams every event unauthenticated. Identity is the \
         caller's claim. The origin guard still refuses a browser page (any Origin but loopback or \
         same-origin), so this is reachable by anything that can make an HTTP request without one — \
         a host on your network. Put a token or an IdP in front of it before this faces anything but \
         a throwaway network (#1650)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                header::HeaderName::from_bytes(name.as_bytes()).expect("header name"),
                HeaderValue::from_str(value).expect("header value"),
            );
        }
        map
    }

    fn body_route(pairs: &[(&str, &str)]) -> OriginDecision {
        decide(&headers(pairs), true)
    }

    #[test]
    fn a_non_browser_client_is_allowed_because_the_quickstart_must_not_pay_for_this() {
        // No Origin at all: curl, an SDK, the Action entrypoint.
        assert_eq!(
            body_route(&[("content-type", "application/json")]),
            OriginDecision::Allowed
        );
    }

    #[test]
    fn a_loopback_origin_is_allowed() {
        for origin in [
            "http://localhost:3000",
            "http://127.0.0.1:5173",
            "http://[::1]:8080",
        ] {
            assert_eq!(
                body_route(&[
                    ("origin", origin),
                    ("content-type", "application/json; charset=utf-8")
                ]),
                OriginDecision::Allowed,
                "{origin} is loopback"
            );
        }
    }

    #[test]
    fn a_same_origin_page_is_allowed_whatever_the_host_is() {
        assert_eq!(
            body_route(&[
                ("origin", "https://rigorix.example.com"),
                ("host", "rigorix.example.com"),
                ("content-type", "application/json"),
            ]),
            OriginDecision::Allowed
        );
    }

    /// The audit's executed case, stated as the acceptance criterion: a page on any origin, sending
    /// the content type a browser can send **without a preflight**, is refused.
    #[test]
    fn a_browser_simple_cross_origin_request_is_refused() {
        let decision = body_route(&[
            ("origin", "http://example.invalid"),
            ("content-type", "text/plain"),
        ]);
        match decision {
            OriginDecision::Refused { reason } => {
                assert!(
                    reason.contains("example.invalid"),
                    "the refusal names the page's origin: {reason}"
                );
                assert!(
                    reason.contains("cross-origin") || reason.contains("preflight"),
                    "the refusal states the rule it applied: {reason}"
                );
            }
            other => panic!("a cross-origin simple request must be refused, got {other:?}"),
        }
    }

    /// The second clause on its own: a cross-origin *JSON* request is refused by the origin rule, and
    /// a loopback browser is not restricted to JSON for nothing — it is the preflight-free shape that
    /// is refused, so a loopback page sending `text/plain` is refused too.
    #[test]
    fn the_content_type_clause_is_about_the_shape_not_the_origin() {
        let cross_origin_json = body_route(&[
            ("origin", "http://evil.example"),
            ("content-type", "application/json"),
        ]);
        assert!(matches!(cross_origin_json, OriginDecision::Refused { .. }));

        let loopback_plain = body_route(&[
            ("origin", "http://localhost:3000"),
            ("content-type", "text/plain"),
        ]);
        match loopback_plain {
            OriginDecision::Refused { reason } => {
                assert!(reason.contains("application/json"), "{reason}")
            }
            other => panic!("a browser sending text/plain can skip the preflight: {other:?}"),
        }
    }

    /// `null` is a sandboxed iframe, a `data:` document or a redirect — it is not "no origin", which
    /// is why the old `/mcp` guard's `origin == "null" => allow` was wrong.
    #[test]
    fn a_null_origin_is_refused_because_it_is_not_the_absence_of_one() {
        match body_route(&[("origin", "null"), ("content-type", "application/json")]) {
            OriginDecision::Refused { reason } => {
                assert!(
                    reason.contains("sandboxed iframe") || reason.contains("null"),
                    "{reason}"
                );
            }
            other => panic!("Origin: null must not read as a non-browser client: {other:?}"),
        }
    }

    /// `/events` carries no body, so the content-type clause does not apply to it — but the origin
    /// clause does, which is the whole point of one decision.
    #[test]
    fn the_event_stream_takes_the_origin_clause_but_not_the_body_clause() {
        assert_eq!(
            decide(&headers(&[("origin", "http://localhost:3000")]), false),
            OriginDecision::Allowed
        );
        assert!(matches!(
            decide(&headers(&[("origin", "http://evil.example")]), false),
            OriginDecision::Refused { .. }
        ));
    }

    #[test]
    fn an_unparseable_origin_is_refused_rather_than_read_as_absent() {
        match body_route(&[
            ("origin", "not-a-url"),
            ("content-type", "application/json"),
        ]) {
            OriginDecision::Refused { reason } => {
                assert!(
                    reason.contains("cannot be shown to be loopback"),
                    "{reason}"
                )
            }
            other => panic!("an origin that cannot be read must not be allowed: {other:?}"),
        }
    }

    #[test]
    fn the_free_mode_warning_fires_beyond_loopback_and_only_when_unarmed() {
        for bind in [
            "127.0.0.1:3001",
            "localhost:3001",
            "[::1]:3001",
            "0:0:0:0:0:0:0:1",
        ] {
            assert!(
                free_mode_warning(bind, false).is_none(),
                "{bind} is loopback and must not warn"
            );
        }
        let warning = free_mode_warning("0.0.0.0:3001", false).expect("a non-loopback bind warns");
        assert!(warning.contains("0.0.0.0:3001"), "{warning}");
        assert!(
            warning.contains("rigorix.execute") && warning.contains("/events"),
            "the line names what becomes reachable, not that it is 'insecure': {warning}"
        );
        assert!(
            free_mode_warning("0.0.0.0:3001", true).is_none(),
            "with an IdP configured the free-mode non-claim does not apply"
        );
    }
}
