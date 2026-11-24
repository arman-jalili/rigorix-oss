//! Every mounted route declares its policy, and the router is built **from** this table (#1659).
//!
//! @canonical .pi/architecture/modules/rigorix-server.md#http-surface
//!
//! # Why this exists
//!
//! The audit's most expensive finding on both repositories is one shape: **a control implemented on
//! one of several routes.** In this repo `/mcp` had an origin guard and `/rpc` had none; in the
//! enterprise repo the OAuth callback's exemption matched a substring of any query. Neither was
//! visible to any gate, because nothing in either tree asserted that a route has a policy *at all*.
//!
//! #1650 fixed the instance. This is the table that makes the class fail a build: the router is
//! constructed by iterating [`ROUTES`], so **mounting a route without a row is not expressible** —
//! there is no second place to add one. A handler that forgets to consult its row is caught by the
//! per-route assertion in `server/tests/route_policy.rs`.
//!
//! # The table is data a reader can see
//!
//! A test failure prints the row it is about, because the row is a value:

use axum::http::HeaderMap;
use axum::response::Response;

/// What kind of authentication a route requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthPolicy {
    /// Nothing: the response carries no consequence beyond a constant.
    None,
    /// The route refuses when an IdP is configured and the caller is not authenticated; with no IdP
    /// (free mode) it answers, deliberately, and says so in the docs.
    SessionWhenConfigured,
    /// Authorization is decided **per method** by the frozen catalog (ADR-0001): the route only
    /// dispatches, so its policy is "whatever the catalog says for this method name".
    PerMethod,
}

/// Whether a route takes the origin decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginPolicy {
    /// The route consults `crate::origin` — the shared decision (#1650).
    Guarded,
    /// The route is exempt, and the row says why. There is exactly one such route today.
    Exempt(&'static str),
}

/// One mounted route's declared policy.
#[derive(Debug)]
pub struct RoutePolicy {
    pub path: &'static str,
    /// The HTTP methods mounted on this path.
    pub methods: &'static [&'static str],
    pub auth: AuthPolicy,
    pub origin: OriginPolicy,
    /// Accepted request content types. **Empty means the route carries no body**, which is what makes
    /// the preflight-free shape inapplicable to it rather than merely unchecked.
    pub content_types: &'static [&'static str],
    /// Why this row says what it says — printed in the failure output of the table test.
    pub rationale: &'static str,
}

impl RoutePolicy {
    /// Whether this route accepts a request body.
    pub fn body_carrying(&self) -> bool {
        !self.content_types.is_empty()
    }

    /// Enforce the declared origin policy for one request, or `None` when it may proceed.
    ///
    /// A route that declares `Guarded` cannot forget: the decision is taken from the row, and the row
    /// is what the router was built from.
    pub fn enforce_origin(&self, headers: &HeaderMap) -> Option<Response> {
        match self.origin {
            OriginPolicy::Exempt(_) => None,
            OriginPolicy::Guarded => crate::origin::refusal(
                crate::origin::decide(headers, self.body_carrying()),
                self.path,
            ),
        }
    }
}

/// The mounted routes. **This is the router's input**, not a description of it.
pub const ROUTES: &[RoutePolicy] = &[
    RoutePolicy {
        path: "/health",
        methods: &["GET"],
        auth: AuthPolicy::None,
        origin: OriginPolicy::Exempt(
            "the response is the constant `ok`: it names no state, starts nothing and leaks nothing, \
             so a page learning that the server is up is not a consequence this decision protects",
        ),
        content_types: &[],
        rationale: "liveness probe: no body, no consequence, no policy to apply",
    },
    RoutePolicy {
        path: "/rpc",
        methods: &["POST", "OPTIONS"],
        auth: AuthPolicy::PerMethod,
        origin: OriginPolicy::Guarded,
        content_types: &["application/json"],
        rationale: "JSON-RPC: the catalog decides authorization per method; the body is JSON so a \
                    browser must preflight (#1650)",
    },
    RoutePolicy {
        path: "/events",
        methods: &["GET"],
        auth: AuthPolicy::SessionWhenConfigured,
        origin: OriginPolicy::Guarded,
        content_types: &[],
        rationale: "SSE stream: no body, and in free mode it is unauthenticated by design — \
                    documented as a non-claim in docs/deployment.md",
    },
    RoutePolicy {
        path: "/mcp",
        methods: &["POST", "GET", "DELETE", "OPTIONS"],
        auth: AuthPolicy::PerMethod,
        origin: OriginPolicy::Guarded,
        content_types: &["application/json"],
        rationale: "Streamable HTTP adapter over the same catalog: per-method authorization, JSON \
                    bodies, the same origin decision",
    },
];

/// The preflight refusal for `path`, taken from its row.
///
/// The two `OPTIONS` handlers used to name `origin::preflight_refusal` directly, which is the same
/// shape of half-measure this module exists to remove: the row declared `Guarded` and the handler did
/// not ask it. There is no arm that answers "nothing": a route with no row, and an exempt route that
/// someone sent a preflight to, each get a refusal that says which of the two it is — an absent
/// answer here would be a preflight granted by omission.
pub fn preflight_refusal_for(path: &str) -> Response {
    use axum::response::IntoResponse;

    match policy_for(path) {
        Some(route) => match route.origin {
            OriginPolicy::Guarded => crate::origin::preflight_refusal(route.path),
            OriginPolicy::Exempt(reason) => (
                axum::http::StatusCode::METHOD_NOT_ALLOWED,
                format!(
                    "{path} has no OPTIONS handler and takes no origin decision: it is exempt \
                     because {reason}"
                ),
            )
                .into_response(),
        },
        None => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("{path} is not declared in route_policy::ROUTES (#1659)"),
        )
            .into_response(),
    }
}

/// The row for `path`, if the route is declared.
pub fn policy_for(path: &str) -> Option<&'static RoutePolicy> {
    ROUTES.iter().find(|route| route.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_complete_and_explains_itself() {
        for route in ROUTES {
            assert!(!route.methods.is_empty(), "{} mounts no method", route.path);
            assert!(
                !route.rationale.trim().is_empty(),
                "{} has no rationale — a row without a reason is a row nobody can review",
                route.path
            );
            if let OriginPolicy::Exempt(reason) = route.origin {
                assert!(
                    reason.len() > 40,
                    "{} is exempt from the origin decision and the reason must say why: {reason}",
                    route.path
                );
            }
            if route.body_carrying() {
                assert!(
                    route.content_types.iter().all(|ct| ct.contains('/')),
                    "{} declares a content type that is not a media type: {:?}",
                    route.path,
                    route.content_types
                );
            }
        }
    }

    #[test]
    fn only_the_constant_route_is_exempt_from_the_origin_decision() {
        let exempt: Vec<&str> = ROUTES
            .iter()
            .filter(|route| matches!(route.origin, OriginPolicy::Exempt(_)))
            .map(|route| route.path)
            .collect();
        assert_eq!(
            exempt,
            vec!["/health"],
            "an exemption is a decision: exactly one route may make it, and adding another means \
             explaining what a page gains by reaching it"
        );
    }

    #[test]
    fn paths_are_unique_and_absolute() {
        let mut paths: Vec<&str> = ROUTES.iter().map(|route| route.path).collect();
        let count = paths.len();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), count, "a path is declared twice");
        assert!(
            paths.iter().all(|path| path.starts_with('/')),
            "every route path is absolute: {paths:?}"
        );
    }
}
