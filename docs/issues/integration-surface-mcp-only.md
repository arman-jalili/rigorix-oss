---
guardian_issue:
  id: "ISSUE-INTEGRATION-SURFACE-MCP-ONLY"
  title: "integration: the MCP gateway is the only adapter surface — record whether that is the ceiling or a gap"
  epic: "docs hygiene"
  component: "MCP"
  module: "mcp-server"
  status: planned
  priority: low
  dependencies: []

  intent: |
    The integration story is the MCP gateway: 15 tools, a real stdio integration test that spawns the
    binary, and an HTTP route. There is no adapter for any agent framework, and the docs do not say
    whether that is by design. An evaluator cannot tell a deliberate single-surface design from an
    unbuilt roadmap, so the honest move is to state which one it is.

  in_scope:
    - "Record the decision: is MCP the integration ceiling, or is a framework adapter expected"
    - "If MCP is the ceiling, say so in the claim ceiling section, with what an adopter must write themselves"
    - "If it is a gap, scope one adapter and name the acceptance test"

  out_of_scope:
    - "Building an adapter speculatively"
    - "Changing the frozen tool catalog"

  affected_layers:
    domain:
      - "None"
    application:
      - "mcp-server"
    infrastructure:
      - "None"
    api:
      - "MCP JSON-RPC over stdio and HTTP"

  canonical_references:
    - evidence: "mcp/src/host/mod.rs:1867 all_tool_descriptors — 6 execution + 4 audit + 4 template + 1 usage = 15"
    - evidence: "mcp/tests/stdio_integration_test.rs — 398 lines, spawns CARGO_BIN_EXE_rigorix-mcp and speaks JSON-RPC over pipes"
    - evidence: "no framework-specific adapter crate exists in the workspace"

  acceptance_criteria:
    - "The claim ceiling or a decision record states whether MCP is the only supported surface"
    - "An adopter can tell what they must write themselves to connect a given host"
    - "No doc implies adapter coverage that does not exist"

  validators:
    - canonical
    - tests

  implementation_notes: |
    This is a decision issue, not a defect report. The end-to-end coverage that exists is real: the
    stdio test drives the actual binary through the actual protocol, so "integration is unverified"
    would be false and must not be written anywhere.

    The distinction worth recording is breadth versus depth. Depth is demonstrated for one surface;
    breadth is zero. A reader deciding whether to adopt needs the second number, and it is currently
    unstated rather than negative.

  file_changes:
    - "modify: mcp/README.md (state the supported surface)"
    - "modify: README.md (claim ceiling, if MCP is the ceiling)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-INTEGRATION-SURFACE-MCP-ONLY

## What exists

| element | verified |
|---|---|
| tool catalog | 15 tools (6 execution, 4 audit, 4 template, 1 usage guide) |
| stdio transport | real: \`Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))\`, piped stdin/stdout, initialize handshake, 398 lines |
| HTTP transport | \`server/src/mcp_http.rs\` |
| framework adapters | none |

## What is unstated

Depth is proven for one surface. Breadth beyond it is zero, and the docs neither claim nor disclaim
it. For an evaluator that is the worst of both: a reader can find the 15 tools and a passing stdio
test, and still not know whether their host is supported.

The fix is a sentence, not a feature — unless the decision is that a framework adapter is in scope,
in which case it becomes a scoped issue with an acceptance test.
