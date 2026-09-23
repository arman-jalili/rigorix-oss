# refactor(mcp): extract host composition into the library (#888 PR A)

Part of #888 (OSS-C2), first of the agreed sequence:
**A** pure-move extraction (this PR) → B typed `HostError` + server skeleton →
C execution methods → D audit/template methods → E SSE.

## Why

The catalog behavior lives in `rigorix-mcp`'s **binary** (`main.rs`):
`AppState::handle_tool_call(name, params) -> Result<Value, Value>` plus the
`build_real_engine` composition. The new `rigorix-server` native API host must
behave identically to the MCP tools and cannot duplicate DTOs or orchestration
glue. `handle_tool_call` is already adapter-neutral (name + JSON → JSON), so
the fix is to move it into the **library**, where both adapters can share it.

A separate host crate is not viable: it would depend on `rigorix-mcp` while
`rigorix-mcp`'s bin depends on it → package cycle.

## What moved (into `mcp/src/host/`)

- `AppState` (+ `new`, `handle_tool_call`), `AppStateExecutor`
- `build_real_engine`, `build_auth_handler`
- `all_tool_descriptors`, `error_type_name`
- `resolve_resource`
- engine/envelope/permission helpers
  (`resolve_approval_identity`, `build_envelope_from_run`, `load_toml_config`,
  `resolve_mcp_permission_mode`, `load_mcp_hook_runner`, `mock_classifier`)
- `APP_STATE` / `app_state()`
- the `approval_binding_tests` + `catalog_drift_tests` test modules

`main.rs` keeps only MCP protocol framing (initialize / tools / resources /
prompts + stdio loop) and delegates through `host::app_state()`.

## Purity

This is a **pure move**. A diff of the moved region against
`origin/main:mcp/src/main.rs` shows only:
- visibility widening (`struct AppState` → `pub struct AppState`,
  `fn new` → `pub fn new`, `async fn handle_tool_call` →
  `pub async fn handle_tool_call`, `all_tool_descriptors`/`error_type_name`/
  `build_real_engine`/`build_auth_handler`/`load_toml_config`/`app_state`/
  `APP_STATE` → `pub`), needed because the binary and server now consume them
- one hoisted `use rigorix_engine::configuration::domain::config::Config`
- rustfmt reflow of two signatures
- the two moved test modules

**No function body changed.** `#[allow(clippy::too_many_arguments)]`-free,
no new dependencies (`mcp/Cargo.toml` untouched).

## Guardrails

- No new deps in `mcp/Cargo.toml` ✅ (file untouched)
- stdio behavior / tool set / enterprise-proxy conditional registration
  untouched ✅
- `cargo test -p rigorix-mcp` ✅ (lib 159 incl. the 6 moved tests; bin 0)
- `RIGORIX_SDK_SCHEMAS=… cargo test -p rigorix-mcp --lib catalog_drift` ✅
- `cargo clippy -p rigorix-mcp --all-targets -- -D warnings` ✅
- `cargo fmt --check` ✅
- `stdio_integration_test` + `e2e_execute_to_audit_test` ✅ (end-to-end
  dispatcher parity through the binary)

## Next

PR B adds a typed `HostError { taxonomy_type, rule_id, step, details }` at the
shared dispatch boundary (MCP formats it back to the current text verbatim;
the server maps it to the `errors.json` taxonomy), then the `server/` crate
skeleton (transport, auth, `system.version`/`usageGuide`, taxonomy mapping).
