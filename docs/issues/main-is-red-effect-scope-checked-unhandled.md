---
guardian_issue:
  id: "ISSUE-MAIN-RED-EFFECT-SCOPE-CHECKED"
  title: "main is red: ExecutionEvent::EffectScopeChecked was added without updating the two event bridges, failing 9 CI and Hardening jobs"
  epic: "event delivery"
  epic_id: "EPIC-NOTIFICATIONS-CORRECTNESS"
  tracking_issue: 1195
  component: "Cli"
  module: "event-system"
  status: planned
  priority: critical
  created_at: "2026-10-24T06:00:00Z"
  updated_at: "2026-10-24T06:00:00Z"

  intent: |
    Restore a green main. One new enum variant breaks three exhaustive matches in two crates.

    `ExecutionEvent::EffectScopeChecked` (`engine/src/event_system/domain/event.rs:338`, added by
    #1189) is handled inside the engine, but the consumers that match on the whole enum were not
    updated:

    - `cli/src/tui/event_bridge.rs:279`
    - `server/src/event_bridge.rs:47`
    - `server/src/event_bridge.rs:102`

    All three fail with `error[E0004]: non-exhaustive patterns`, and `-D warnings` turns that into a
    build failure. The cascade on `8b709e40`:

    | run | failing jobs |
    |---|---|
    | CI | Lint (cli), Lint (server), Final Gate |
    | Hardening | Static Analysis, Integration, Unit (cli), Unit (server), Lint (cli), Lint (server), Build (cli), Build (server), Release Readiness |

    Nine jobs, one root cause, and `Final Gate` is among them — so main is not mergeable and the
    release train is stopped.

    **Why it escaped:** the local verification was engine-scoped — `local-ci (engine) 71/71`,
    `cargo clippy --lib --tests` inside the engine crate, and `cargo test --lib approval`. All true, and
    all blind to the two crates that consume the enum. Adding a variant to a public enum is a
    workspace-wide event; the check that would have caught it is `cargo check --workspace --all-targets`.

  dependencies:
    - name: "ExecutionEvent"
      type: internal
      note: "engine/src/event_system/domain/event.rs — the closed vocabulary (#1189 added the variant)."
    - name: "the two event bridges"
      type: internal
      note: "cli/src/tui/event_bridge.rs, server/src/event_bridge.rs — exhaustive matches over the enum."

  in_scope:
    - "Add the missing arm in all three matches (map to a TUI/server event, or explicitly ignore with a reason)"
    - "Re-run Lint (cli), Lint (server), Build and Unit for both crates locally before closing"
    - "Add a workspace-wide compile gate to the local workflow so an enum change cannot pass engine-only checks"

  out_of_scope:
    - "Changing the engine-side handling of EffectScopeChecked"
    - "Broadcast/SSE semantics of the new event (it may legitimately be ignored by the bridges)"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "None"
    api:
      - "Modified: cli and server event bridges"

  canonical_references:
    - module: ".pi/architecture/modules/event-system.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-24"
    - evidence: "engine/src/event_system/domain/event.rs:338,532,559,591 — the variant and its engine arms"
    - evidence: "cli/src/tui/event_bridge.rs:279 — E0004"
    - evidence: "server/src/event_bridge.rs:47,102 — E0004"
    - evidence: "gh run list: CI and Hardening failure on 8b709e40"

  acceptance_criteria:
    - "cargo check --workspace --all-targets is clean"
    - "Lint (cli), Lint (server), Build (cli), Build (server) pass in CI"
    - "CI and Hardening are green on main"
    - "Each new arm states why the event is or is not surfaced to the UI/event stream"
    - "A workspace compile gate exists locally (documented command or script)"

  validators:
    - ci
    - architecture-validator

  implementation_notes: |
    Deciding what the bridges should *do* with the event is a real question, not a mechanical one: it is
    recorded when a scope check runs, which the TUI might show as a step annotation and the server's
    stream might forward. Whatever you choose, write the reason next to the arm — a bare `_ => {}` is
    how the next variant gets ignored by accident.

    Do not add a catch-all arm to silence the compiler. The exhaustiveness error is the guard that
    forced this issue to exist; a wildcard would disable it for every future variant, which is the same
    class as `#[serde(other)]` on a closed vocabulary.

  file_changes:
    - "modify: cli/src/tui/event_bridge.rs"
    - "modify: server/src/event_bridge.rs"
    - "modify: .pi/scripts/local-ci.sh or docs (workspace compile gate)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1195: main is red from one unhandled variant

## What you are looking at

```
engine: ExecutionEvent::EffectScopeChecked        ← added (#1189)
cli:    match … { … }  at event_bridge.rs:279     ← not exhaustive  ✗
server: match … { … }  at event_bridge.rs:47,102  ← not exhaustive  ✗
```

Nine CI/Hardening jobs fail; `Final Gate` is one of them.

## In Scope

- Three arms; local builds of both crates; a workspace-wide compile gate

## Out of Scope

- Engine-side semantics; what the stream should carry

## Affected Layers

### API
- cli + server event bridges

## Canonical References

- **Module:** `.pi/architecture/modules/event-system.md`
- **Evidence:** `event.rs:338`; `cli/.../event_bridge.rs:279`; `server/.../event_bridge.rs:47,102`; the failing runs

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Workspace check clean | `cargo check --workspace --all-targets` |
| 2 | Lint/Build for cli+server pass | CI |
| 3 | CI + Hardening green on main | CI |
| 4 | Each arm has a stated reason | review |
| 5 | Local workspace gate exists | review |

## Implementation

1. Read the three matches and the engine's own arms for the variant.
2. Add arms with reasons.
3. Run `cargo check --workspace --all-targets`, then the two crates' lint/build jobs.

## Pitfalls

- **Do not add `_ => {}`** — that disables the exhaustiveness guard for every future variant.
- **Do not trust engine-only verification** for a change to a public enum.
- **Check `Final Gate`**: it is the job that says main is mergeable.

## Why this is critical

Main is red and unmergeable, and the cause is a one-line-per-site omission that a workspace compile
would have caught before the push.
