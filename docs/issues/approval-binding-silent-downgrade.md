---
guardian_issue:
  id: "ISSUE-APPROVAL-BINDING-SILENT-DOWNGRADE"
  title: "approval: RIGORIX_APPROVAL_BINDING=1 with no HMAC key logs a warning and silently falls back to unbound approval"
  epic: "consequence gating"
  component: "ApprovalBindingSetup"
  module: "execution-engine"
  status: planned
  priority: high
  dependencies:
    - "ADR-011 — intent-bound approval"

  intent: |
    A requested security guarantee must not evaporate because its key material is missing. When
    the operator sets `RIGORIX_APPROVAL_BINDING=1`, they are asking for intent-bound approval —
    approval that binds the tool, the resolved payload and the declared scope, and re-verifies
    the digest at dispatch.

    If `RIGORIX_HMAC_KEY` is absent the setup logs a warning and returns `None`, so the legacy
    unbound approval path stays active. The process starts, looks healthy, and approves things
    without binding them. Nothing in the API surface reports the downgrade.

  in_scope:
    - "Treat an explicit enable with missing key material as a startup failure"
    - "Or refuse the affected operations rather than falling back"
    - "Distinguish 'not requested' from 'requested but unsatisfiable' in the type"
    - "Surface the active approval mode in a place an operator can read"
    - "Test: enable without a key fails loudly; enable with a key binds"

  out_of_scope:
    - "Changing the binding algorithm or canonicalisation"
    - "Key distribution / rotation"
    - "The general 'security posture report' idea — this is the concrete silent-downgrade defect"

  affected_layers:
    domain:
      - "Modified: approval binding setup — fallible construction"
    application:
      - "Modified: composition roots (CLI, MCP, Actions) — propagate the error"
    infrastructure:
      - "None"
    api:
      - "Modified: startup fails instead of warning"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - evidence: "engine/src/execution_engine/application/factory.rs:212 — reads RIGORIX_HMAC_KEY"
    - evidence: "engine/src/execution_engine/application/factory.rs:217-218 — tracing::warn!('RIGORIX_APPROVAL_BINDING=1 but RIGORIX_HMAC_KEY is unset — approval binding disabled') then None"

  acceptance_criteria:
    - "Explicit enable + missing key does not start with binding disabled"
    - "The failure names the missing variable and the consequence"
    - "Unset/0 still yields the legacy path with no error"
    - "A test asserts the failure, and that a valid key still binds"
    - "The active approval mode is observable at startup"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - The existing behaviour is defensible for a dev tool but wrong for a control that an
      operator deliberately turned on. The distinguishing signal is intent: 'unset' means the
      operator did not ask; '=1 with no key' means they asked and cannot be satisfied.
    - Return a typed error, not `None`. `None` is indistinguishable from 'not requested', which
      is why this cannot be detected downstream.
    - The same pattern appears in the precondition/capacity setup paths where a malformed
      declaration is already a hard error. Match that: malformed or unsatisfiable configuration
      fails at composition time.

  file_changes:
    - "modify: engine/src/execution_engine/application/factory.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs"
    - "modify: mcp/src/host/mod.rs"
    - "modify: actions/src/main.rs"
    - "create: engine/tests/unit/approval/approval_binding_activation_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-APPROVAL-BINDING-SILENT-DOWNGRADE

## The defect

```rust
// factory.rs:212-218
let run_key = std::env::var("RIGORIX_HMAC_KEY")
    ...
    .map_err(|_| {
        tracing::warn!(
            "RIGORIX_APPROVAL_BINDING=1 but RIGORIX_HMAC_KEY is unset — approval binding disabled"
        )
    })
```

An operator who set `RIGORIX_APPROVAL_BINDING=1` gets a warning on stderr and an unbound
approval path. The process starts and serves.

## Why a warning is not enough

| signal | who sees it | when |
|---|---|---|
| `tracing::warn!` | whoever reads the log | at startup, once |
| `None` returned | the composition root | silently |
| API surface | nobody | never |

So a deployment can be configured for intent binding, run for months, and be approving
unbound — with the only evidence a log line someone may not have kept.

This is the concrete instance of the general principle: **a requested guarantee must not
silently disappear.** Unset means "not requested" and should stay quiet; `=1` without a key
means "requested and unsatisfiable" and must fail.

## Suggested fix

Make the setup fallible and let the composition roots propagate:

```rust
pub fn from_env() -> Result<Option<ApprovalBinding>, ApprovalBindingError>
```

- `Ok(None)` — not requested.
- `Err(MissingKey { var, consequence })` — requested, cannot be honoured.
- `Ok(Some(..))` — armed.

This matches the precedent already set for capacity declaration: a malformed declaration is a
hard error at composition time, not a silent downgrade.
