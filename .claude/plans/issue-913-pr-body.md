# fix: #913 OSS-SERVER-SSE-AUTH — gate `GET /events`

`GET /events` streamed run events with **no session gate**, while `POST /rpc`
enforced the ADR-0001 D4 auth level per method. A non-loopback
`RIGORIX_SERVER_BIND` therefore exposed an unauthenticated event stream. This
applies the **same** gate the RPC path uses (option **A** in the issue).

## What changed
- `server/src/events.rs` — `events_handler` now resolves auth **once at
  connect**, before subscribing:
  ```rust
  if state.backend.auth_configured().await && !state.backend.is_authenticated().await {
      return (StatusCode::UNAUTHORIZED, "A session is required for the event stream — run rigorix.auth.login").into_response();
  }
  ```
  Reuses `backend.auth_configured()` / `backend.is_authenticated()` — **no
  second auth path**.
- `server/.pi/architecture/modules/rigorix-server.md` — documents the posture.
- Tests: refused (unauthenticated) and allowed (authenticated) when an IdP is
  configured; the existing no-IdP SSE path is unchanged.

## Acceptance criteria

| # | Criterion | Evidence |
|---|-----------|----------|
| 1 | Requires a valid session when an IdP is configured (fail closed) | `events_handler_refuses_unauthenticated_when_idp_configured` (401) |
| 2 | Matches the RPC path exactly; no-IdP short-circuit preserved | `events_handler_streams_text_event_stream` + `events_handler_allows_authenticated_when_idp_configured` |
| 3 | A test proves the chosen behaviour | the three handler tests |
| 4 | `rigorix-server.md` documents the posture | docs update |
| 5 | CI green (fmt/clippy/tests) | below |

## Validation

```
cargo test -p rigorix-server                                 ✅ 6 suites, 0 failed
cargo clippy -p rigorix-server --all-targets -- -D warnings  ✅
cargo fmt --all --check                                      ✅
```

## Notes
- **Option A** chosen over localhost-only (B) because it mirrors the RPC gate and
  keeps `RIGORIX_SERVER_BIND` flexible; the no-IdP local mode is preserved, so
  local deployments keep the stream.
- No new auth path; no per-event checks.

Fixes #913
