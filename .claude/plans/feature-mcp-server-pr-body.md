# feat(mcp): real LLM planning, SSE removal, session handshake, e2e hygiene

**Batch 7** of the gap-ledger implementation backlog (`feature/mcp-server`).
Closes **#724** (GAP-A-07), **#727** (GAP-A-10), **#739** (GAP-A-22), **#742** (GAP-A-25).

## #724 — real LLM planning in `build_real_engine`
- Real Claude/OpenAI classifiers + `LlmParameterExtractor` wired (env: `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `RIGORIX_PLANNING_MODEL`)
- Mock mode only via explicit `RIGORIX_MOCK_PLANNING=1` or when no key is set (warn-logged)
- Default template `FileRead` no longer reads `/dev/null` — resolves `repo_root/README.md`

## #727 — SSE removed
`--sse` never started a real server (it logged "not fully implemented" and exited). Server now always runs stdio; `--sse`/`--bind` accepted with a deprecation notice.

## #739 — initialize creates a real session
The handshake previously emitted a throwaway `SessionId` event and never wrote the session repository. `initialize` now persists a `Session` via `session_repo.save` and the event carries its real id.

## #742 — e2e hygiene
Shared `minify_json` / `send_rpc` / `poll` helpers extracted into `tests/common/mod.rs` (previously duplicated in three files with divergent sleeps); all three test files refactored.

## Bonus (AC closure)
- **#734 AC1**: `AuditEvent` domain enum is now referenced from production code — the sender constructs `AuditEvent` values and converts via `From<AuditEvent> for ExecutionEvent` (previously the enum was dead). `ExecutionEvent` gains `AuditEnvelopeCreated` for totality.
- **#756 AC1**: regression test — `planning_prompt_content` populated deterministically only when `capture_planning_prompt` is enabled.

## Verification
- engine 1901 lib tests, workspace 2507 total, mcp lib + all e2e/integration targets
- clippy `-D warnings`, fmt clean, workspace builds
