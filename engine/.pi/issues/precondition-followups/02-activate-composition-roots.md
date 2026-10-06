---
guardian_issue:
  id: "ISSUE-PF-ACT-2"
  title: "ACT: wire precondition setup into composition roots (MCP/CLI/actions/server)"
  epic: "precondition-followups"
  epic_id: "EPIC-PRECONDITION-FOLLOWUPS"
  component: "CompositionRoots"
  module: "precondition"
  status: planned
  priority: critical
  dependencies:
    - "ISSUE-PF-ACT-1"

  in_scope:
    - "mcp/src/host/mod.rs (shared host — also used by rigorix-server via init_host)"
    - "cli/src/cli_boundary/orchestrator.rs"
    - "actions/src/main.rs"
    - "Decide/document the actions path (it does not load rigorix.toml today)"
    - "Update the runbook with how to enable preconditions"

  out_of_scope:
    - "New precondition behavior"
    - "Server-specific code (it composes through the MCP host)"

  affected_layers:
    api:
      - "Modify: mcp/src/host/mod.rs"
      - "Modify: cli/src/cli_boundary/orchestrator.rs"
      - "Modify: actions/src/main.rs"

  canonical_references:
    - module: "engine/.pi/architecture/modules/consequence-gating.md#dispatch-integration"
    - code: "mcp/src/host/mod.rs:1351"
    - code: "cli/src/cli_boundary/orchestrator.rs:318"
    - code: "actions/src/main.rs:395"

  acceptance_criteria:
    - "MCP host builds PreconditionSetup::from_env(repo_root) and passes it in the factory config"
    - "CLI orchestrator does the same from the resolved project root"
    - "actions either loads the config or explicitly documents that preconditions are unsupported there (fail-closed posture documented)"
    - "server inherits the wiring through rigorix_mcp::host::init_host (verified by a smoke test)"
    - "runbook documents enabling via .rigorix/preconditions.toml"

  validators:
    - ci
    - tests
    - architecture
    - canonical
    - integration

  implementation_notes: |
    Mirror how ApprovalBindingSetup::from_env and SequencePolicySetup::from_env
    are already threaded at the three factory sites. The MCP host is the shared
    composition root: rigorix-server calls init_host, so wiring it there covers
    the server. actions/src/main.rs currently does not read rigorix.toml — decide
    and record the decision (load it, or document unsupported + why fail-open is
    acceptable there).

  file_changes:
    - "modify: mcp/src/host/mod.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs"
    - "modify: actions/src/main.rs"
    - "modify: engine/docs/runbook-consequence-gating.md (rename tracked by ISSUE-PF-REN-1)"
---

# ISSUE-PF-ACT-2: Composition roots

## Sites

| Root | Line | Notes |
|------|------|-------|
| `mcp/src/host/mod.rs` | ~1351 | shared host; also backs `rigorix-server` via `init_host` |
| `cli/src/cli_boundary/orchestrator.rs` | ~318 | resolves project config |
| `actions/src/main.rs` | ~395 | does **not** load `rigorix.toml` today |

## Acceptance

Enabling `.rigorix/preconditions.toml` with a matching precondition must produce
an executor holding an armed `PreconditionDispatchGate` in every supported root.

## References

- `ParallelExecutionFactoryConfig` (after ISSUE-PF-ACT-1)
- existing `ApprovalBindingSetup::from_env` / `SequencePolicySetup::from_env` call sites
