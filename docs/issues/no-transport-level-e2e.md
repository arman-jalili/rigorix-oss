---
guardian_issue:
  id: "ISSUE-NO-TRANSPORT-LEVEL-E2E"
  title: "the server's HTTP/SSE surface and the actions entrypoint are closed only at codec/router level — 6 of 190 test files spawn a process, 1 binds a socket"
  epic: "release hygiene"
  epic_id: "EPIC-OPERATIONAL-USABILITY"
  tracking_issue: 1209
  component: "Server"
  module: "operations"
  status: planned
  priority: medium
  created_at: "2026-10-24T12:25:00Z"
  updated_at: "2026-10-24T12:25:00Z"

  intent: |
    Close the two operator-facing surfaces with tests that actually cross the transport boundary.

    Census over every test directory: 190 files, **6 that spawn a process**, **1 that binds a socket**.
    That is not an indictment of the 139 engine files — in-memory fakes are the right tool for domain
    tests. It is a statement about the two surfaces whose whole job is the boundary:

    `server/tests/` (6 files, 0 processes, 0 sockets):
    - `rpc_transport.rs` — a fake `MethodBackend`, explicitly "without a live engine".
    - `events_live.rs` — "A connected SSE subscriber (modelled by `EventHub::subscribe`)": the broadcast
      channel, not an HTTP client reading frames.
    - `mcp_streamable_http.rs` — a real axum `Router` driven by `oneshot`, i.e. no transport.

    `server/src/events.rs` unit-tests the framing logic (Last-Event-ID, heartbeat, backpressure; 8
    tests), so the *logic* is covered. What is not covered is that a client can consume it: status line,
    `content-type: text/event-stream`, chunk boundaries, reconnect carrying `Last-Event-ID`.

    `actions/tests/` (1 file): `inputparser_integration.rs`. Nothing runs the built binary with a
    `GITHUB_*` environment and asserts the emitted outputs and exit code — the action's only contract
    with a workflow.

  dependencies:
    - name: "server HTTP/SSE"
      type: internal
      note: "server/src/{main,rpc,events}.rs"
    - name: "actions entrypoint"
      type: internal
      note: "actions/src/main.rs"
    - name: "in-tree socket pattern"
      type: internal
      note: "mcp/tests/idpclient_integration_test.rs binds 127.0.0.1:0"

  in_scope:
    - "One server test on an ephemeral port: real client, a JSON-RPC call, an SSE frame, and a reconnect with Last-Event-ID"
    - "One actions test that spawns the built binary against a fixture action.yml and asserts outputs"
    - "Both offline: no Docker, no database, no external service"

  out_of_scope:
    - "Moving the engine's 139 in-memory test files onto real transports"
    - "The adapter-surface question (is MCP the only integration surface) — that is #1104, closed"

  affected_layers:
    domain: ["None"]
    application: ["None"]
    infrastructure: ["Modified: server/tests/, actions/tests/"]
    api: ["None"]

  canonical_references:
    - module: ".pi/architecture/modules/operations.md"
    - adr: []
    - last_sync: "2026-10-24"
    - evidence: "census: engine 139/1/0, mcp 42/3/1, cli 2/2/0, server 6/0/0, actions 1/0/0 (files/process/socket)"
    - evidence: "server/tests/events_live.rs:3-6 — 'modelled by EventHub::subscribe'"
    - evidence: "server/tests/rpc_transport.rs:1-4 — 'without a live engine'"
    - evidence: "server/src/events.rs — 8 unit tests over framing"

  acceptance_criteria:
    - "A test binds a port and asserts an SSE frame reaches a real HTTP client"
    - "A test asserts a Last-Event-ID reconnect replays from the right id"
    - "A test spawns the actions binary and asserts its outputs and exit code"
    - "None of them needs Docker, a database, or the network"

  validators:
    - integration
    - tests

  implementation_notes: |
    Bind `127.0.0.1:0` and read the assigned port; the pattern already exists in
    `mcp/tests/idpclient_integration_test.rs`. Do not use a fixed port — parallel tests will collide, and
    a flaky transport test is worse than none.

    For the SSE assertion, read frames with a plain HTTP client and parse `id:`/`data:` lines rather than
    asserting on a stringified body: the point is that the framing survives a socket, which is exactly
    what a `oneshot` test cannot show.

  file_changes:
    - "add: server/tests/events_over_socket.rs"
    - "add: server/tests/rpc_over_socket.rs (or one file covering both)"
    - "add: actions/tests/entrypoint_e2e.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1209: the boundary surfaces are tested without the boundary

## In Scope

- One socket-level server test (RPC + SSE + Last-Event-ID reconnect); one process-level actions test

## Out of Scope

- Re-homing engine tests; the adapter-surface question (#1104, closed)

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | SSE frame over a real socket | integration |
| 2 | Last-Event-ID replay | integration |
| 3 | Actions binary outputs | integration |
| 4 | Offline | CI |

## Implementation

1. Ephemeral port, real client, JSON-RPC call.
2. Same server, open SSE, assert a frame; reconnect with `Last-Event-ID` and assert the replay.
3. Spawn the actions binary with a fixture env and `action.yml`.
