//! #1659's last mile: a route cannot be *enforced* without its row either.
//!
//! `app::router` is built by iterating `route_policy::ROUTES`, so mounting a route without a policy
//! row is not expressible. That closes one direction. It does not close the other: a handler can
//! still be mounted by a row and take its own decision from `crate::origin` directly, which is
//! exactly what `/events` did — the row said `Guarded` and nothing read it.
//!
//! This test is the property, not the instance: it enumerates **the router's own table** and, for
//! every route that declares the origin decision, requires the handler's **body** to ask
//! `policy_for("<its own path>")`. A new route fails the coverage check; a handler that stops asking
//! its row fails the body check; a row that declares `Guarded` but is served inline fails because an
//! inline closure cannot be proved to consult anything.
//!
//! The body is extracted from the function signature to the first closing brace in column 0 — a
//! line-split scan would read the test module's own string literals (this class of scan has matched
//! its own subject twice in this repository) and a whole-file scan would pass on the strength of a
//! comment.

use rigorix_server::route_policy::{AuthPolicy, OriginPolicy, ROUTES};

/// Where each route's handler lives, and what it is called.
///
/// The path column is the router's own: an entry naming a path `ROUTES` does not declare is a
/// failure. A `Guarded` row with **no** entry here is also a failure — that is the `/events` case.
struct Handler {
    path: &'static str,
    /// The module name as `app.rs` writes it, or `None` when the handler is an inline closure.
    module: Option<&'static str>,
    function: Option<&'static str>,
    source: &'static str,
}

const HANDLERS: &[Handler] = &[
    // `/health` is `get(|| async { "ok" })` in `app.rs`: a constant with no request state, exempt
    // from the origin decision for the reason its row gives.
    Handler {
        path: "/health",
        module: None,
        function: None,
        source: include_str!("../src/app.rs"),
    },
    Handler {
        path: "/rpc",
        module: None,
        function: Some("rpc_handler"),
        source: include_str!("../src/app.rs"),
    },
    Handler {
        path: "/events",
        module: Some("events"),
        function: Some("events_handler"),
        source: include_str!("../src/events.rs"),
    },
    Handler {
        path: "/mcp",
        module: Some("mcp_http"),
        function: Some("post_handler"),
        source: include_str!("../src/mcp_http.rs"),
    },
    Handler {
        path: "/mcp",
        module: Some("mcp_http"),
        function: Some("get_handler"),
        source: include_str!("../src/mcp_http.rs"),
    },
    Handler {
        path: "/mcp",
        module: Some("mcp_http"),
        function: Some("delete_handler"),
        source: include_str!("../src/mcp_http.rs"),
    },
];

const ROUTER_SOURCE: &str = include_str!("../src/app.rs");

/// The handler body: from the signature to the first closing brace at the start of a line.
fn body<'a>(handler: &Handler, source: &'a str) -> &'a str {
    let function = handler
        .function
        .unwrap_or_else(|| panic!("{} has no function to extract", handler.path));
    // `async fn`, not `pub async fn`: `/rpc`'s handler is private to `app.rs`, and requiring
    // visibility would make the extractor's rule a style preference rather than a shape.
    let signature = format!("async fn {function}(");
    let start = source.find(&signature).unwrap_or_else(|| {
        panic!(
            "{} names handler `{function}`, which is not an `async fn` in the source this test \
             reads — a renamed handler must be renamed here too",
            handler.path
        )
    });
    let rest = &source[start..];
    let end = rest.find("\n}\n").unwrap_or_else(|| {
        panic!(
            "{}'s handler body has no closing brace in column 0 — is the file rustfmt-formatted?",
            handler.path
        )
    });
    &rest[..end]
}

#[test]
fn every_declared_route_has_a_handler_this_suite_can_inspect() {
    for route in ROUTES {
        assert!(
            HANDLERS.iter().any(|handler| handler.path == route.path),
            "{} is declared in ROUTES and this suite has no handler for it: add its module and \
             function so the property below can be checked (an unchecked route is how /events kept \
             a row nothing read)",
            route.path
        );
    }
    for handler in HANDLERS {
        assert!(
            ROUTES.iter().any(|route| route.path == handler.path),
            "{} is inspected here and not declared in ROUTES — the table is the router's input, and \
             a handler for an undeclared route is a route nothing mounts",
            handler.path
        );
    }
}

#[test]
fn every_guarded_route_asks_its_own_row() {
    for route in ROUTES {
        if !matches!(route.origin, OriginPolicy::Guarded) {
            continue;
        }
        let inspected: Vec<&Handler> = HANDLERS
            .iter()
            .filter(|handler| handler.path == route.path)
            .collect();
        for handler in inspected {
            let function = handler.function.unwrap_or_else(|| {
                panic!(
                    "{} declares `Guarded` and is served by an inline closure in app.rs — an inline \
                     closure cannot be proved to consult its row, so this route's declaration is a \
                     comment",
                    route.path
                )
            });
            let extracted = body(handler, handler.source);
            // Derived from the row, never written in this test: the handler must name the path its
            // row names. A test that hardcoded "/mcp" would prove one route (which is what the
            // per-file assertion in mcp_http.rs does, and why it did not catch /events).
            let expected = format!("policy_for(\"{}\")", route.path);
            assert!(
                extracted.contains(&expected),
                "{} declares `Guarded` and its handler `{function}` never asks its own row: \
                 expected `{expected}` in the body. Taking the decision from `crate::origin` \
                 directly is the half-measure this closes — the row is what the router was built \
                 from, so a row nothing reads cannot be enforced.",
                route.path
            );
            assert!(
                extracted.contains("enforce_origin("),
                "{}'s handler `{function}` asks for its row but does not enforce it",
                route.path
            );
        }
    }
}

#[test]
fn every_mounted_handler_is_reachable_from_the_router_table() {
    // The entries above that live in another module must be the ones `app.rs` mounts, or this suite
    // is inspecting a function nothing serves.
    for handler in HANDLERS {
        let (Some(module), Some(function)) = (handler.module, handler.function) else {
            continue;
        };
        let mount = format!("{module}::{function}");
        assert!(
            ROUTER_SOURCE.contains(&mount),
            "{} names `{mount}`, which app::router does not mount — this suite would be inspecting \
             a handler no route serves",
            handler.path
        );
    }
}

#[test]
fn an_exempt_route_takes_no_origin_decision_at_all() {
    // The exemption is a decision, and the way it goes wrong is that someone adds the guard "for
    // symmetry" while the row still says exempt: two answers to one question.
    for route in ROUTES {
        let OriginPolicy::Exempt(reason) = route.origin else {
            continue;
        };
        for handler in HANDLERS.iter().filter(|h| h.path == route.path) {
            let Some(_) = handler.function else {
                continue; // an inline closure: nothing of ours to scan
            };
            let extracted = body(handler, handler.source);
            assert!(
                !extracted.contains("origin::decide")
                    && !extracted.contains("enforce_origin")
                    && !extracted.contains("origin::refusal"),
                "{} is declared exempt ({reason}) and its handler takes an origin decision anyway",
                route.path
            );
        }
    }
}

#[test]
fn every_row_names_an_authentication_policy_and_a_body_shape() {
    // Cheap, but it is the pair of columns a new row is most likely to leave at a default: `PerMethod`
    // copied from `/rpc` onto a route with no method dispatch, or a body-carrying route with an empty
    // content-type list, which is what makes the preflight-free shape inapplicable.
    for route in ROUTES {
        if matches!(route.auth, AuthPolicy::PerMethod) {
            assert!(
                route.rationale.contains("catalog") || route.rationale.contains("method"),
                "{} uses PerMethod; its rationale must say which catalog decides",
                route.path
            );
        }
    }
    let body_routes: Vec<&str> = ROUTES
        .iter()
        .filter(|route| route.body_carrying())
        .map(|route| route.path)
        .collect();
    assert_eq!(
        body_routes,
        vec!["/rpc", "/mcp"],
        "the body-carrying routes are the two JSON routes; a third needs its content types and a \
         rationale for why a browser cannot reach it as a simple request"
    );
}
