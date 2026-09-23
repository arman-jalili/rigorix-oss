# feat(server): OSS reference execution host skeleton (#888 PR B)

Part of #888 (OSS-C2). Follows the agreed sequence: **PR A** (merged, host
extraction) → **PR B** (this: structured errors + server skeleton) → PR C
(execution methods) → PR D (audit/template methods) → PR E (SSE).

Does **not** close #888.

## 1. Structured `EngineFacadeError` (the real first task)

Per your correction: the structured `OrchestratorError` variants already exist;
`map_orchestrator_error` was flattening them into `Internal(err.to_string())`.

- `EngineFacadeError::{SequencePolicyDenied { rule_id, step }, RequirementUnmet { requirement_id, step, unmet }, IdentityRequired { step, status }}`
  added and populated in `map_orchestrator_error`.
- No engine-side change; no engine contract note.
- This fixes the `run`/`execute` path that previously lost `rule_id`/`step`
  (and `identity_required`), so the native API can emit
  `denied_by_sequence` (`-32010`) with `data.rule_id`/`data.step` and
  `identity_required` (`-32012`) structurally.

## 2. `HostError` at the dispatch boundary

- New `mcp/src/host/error.rs`: `ErrorType` (the `errors.json` `data.type`
  identifiers + JSON-RPC codes) and `HostError { error_type, message, rule_id,
  step, details, retryable }`.
- Maps every `EngineFacadeError` variant, plus the execution/audit/template
  handler errors and `AuthError`.
- `AppState::handle_tool_call` now returns `Result<Value, HostError>`; the MCP
  adapter formats `HostError.message` back to the existing text (the only
  visible MCP change: a sequence denial now reads `Engine error: Sequence
  policy denied …` instead of `Engine error: Internal error: Sequence policy
  denied …`; no test asserted the old flattening).
- `HostError::to_jsonrpc_error()` emits the frozen error object
  (`code`, `message`, `data.type`, optional `rule_id`/`step`/`details`).

## 3. `server/` crate skeleton

New workspace crate `rigorix-server` (binary), added to `Cargo.toml` members.
It is a **host, not a second engine**: `HostBackend` delegates each catalog
method to `rigorix_mcp::host::AppState::handle_tool_call` — the same composed
host as the stdio MCP binary — so results and error taxonomy are identical.

- `POST /rpc`: JSON-RPC 2.0 single / batch / notification; HTTP 200 for a
  well-formed envelope; `-32700` malformed, `-32600` invalid, `-32601` unknown.
- `catalog.rs`: the 19 frozen methods (name → auth → MCP tool); `policy.bundle`
  (admin, no MCP tool) is refused `not_enabled`, never fabricated.
- `auth`: public / session / admin per catalog entry; local mode (no IdP) does
  not refuse; identity stays session-derived (D5).
- `rigorix.system.version`: name / engine version / API version / schemas /
  capabilities.
- `engine::ENGINE_VERSION` exposed so the host reports the real engine version.

## Tests

- `server/tests/rpc_transport.rs` (11): single, batch, notification,
  notification-batch, empty batch, unknown method, wrong version, non-object,
  parse error, `denied_by_sequence` rule_id+step, `system.version`.
- `server/tests/auth_levels.rs` (5): public/session/admin, unattested refusal,
  local mode.
- `server/tests/catalog_parity.rs` (6): 19 unique methods, MCP-tool 1:1
  (`all_tool_descriptors` + auth descriptors), SDK `catalog.json`
  field-for-field when `RIGORIX_SDK_SCHEMAS` is set.
- `mcp` lib tests (incl. the 4 new `host::error` tests) green.

## CI

- `server` added to `.pi/scripts/local-ci.sh` `CRATES`; per-crate
  `check_catalog_parity` / `stage_unit` / `stage_integration` scripts; module
  doc + README workspace layout.
- `bash .pi/scripts/local-ci.sh --crate=server` → 11/11 PASS.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
  `cargo build --workspace` clean.

## Deferred

- **PR C/D**: dedicated typed tests for the already-delegated execution and
  audit/template methods.
- **PR E**: `GET /events` SSE (blocked on SDK-C1).
