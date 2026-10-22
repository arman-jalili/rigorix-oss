---
guardian_issue:
  id: "ISSUE-ENFORCEMENT-PRESET-FLAG-SILENTLY-IGNORED"
  title: "cli: --enforcement is accepted, threaded through three layers, and read by nothing — a security-relevant preset that silently does nothing"
  epic: "enforcement correctness"
  epic_id: "EPIC-ENFORCEMENT-CORRECTNESS"
  tracking_issue: 1156
  component: "Cli"
  module: "enforcement"
  status: planned
  priority: high
  created_at: "2026-10-23T06:00:00Z"
  updated_at: "2026-10-23T06:00:00Z"

  intent: |
    Stop accepting a flag that changes nothing, or make it change something.

    `rigorix run --enforcement <preset>` parses a value, threads it into the run intent, and the
    orchestrator builder stores it on `enforcement_preset`. Nothing ever reads that field. The
    enforcement config used at execution time comes from `Configuration.enforcement` — the
    `.rigorix` config file — resolved once at startup.

    The architecture already records this honestly:

    > `engine/.pi/architecture/modules/enforcement.md:23` — "the per-run `--enforcement` flag
    > (`RunInput::enforcement_preset`) is threaded and not yet read — the executor config is built
    > once at startup, so the flag cannot vary per run. Selecting a preset per run needs a
    > config-override path, not a wiring fix."

    So the architecture is not lying. **The CLI surface is.** A run with `--enforcement strict`
    looks hardened to the operator and is not. That asymmetry — honest doc, dishonest interface — is
    worse than a documented limitation, because the person at risk reads the flag, not the module
    doc.

  dependencies:
    - name: "RunInput::enforcement_preset"
      type: internal
      note: "engine/src/orchestrator/application/dto/mod.rs:50,196; builder_impl.rs:30,196-197. Written in 6 places, read in none."
    - name: "Configuration.enforcement (the real source)"
      type: internal
      note: "cli/src/cli_boundary/orchestrator.rs:395 builds the executor config from `engine_config.enforcement` — the file, not the flag."
    - name: "ParallelExecutionFactoryConfig.enforcement"
      type: internal
      note: "engine/src/execution_engine/application/parallel_executor.rs:84 — the field a per-run override would have to reach; built once at startup."

  in_scope:
    - "Choose: (a) implement a per-run override, (b) reject the flag loudly, (c) remove flag + DTO field + builder method"
    - "Whichever is chosen, make the CLI, the DTO and the module doc say the same thing"
    - "If (b): the error names the config file as the supported path, so the operator can act"
    - "Test that the flag's documented behaviour is the observed behaviour (either it changes the resolved config, or it is refused)"

  out_of_scope:
    - "Redesigning enforcement presets or their limits"
    - "The `[enforcement] preset` config-file setting, which works"
    - "Per-edge or per-tool enforcement overrides"

  affected_layers:
    domain:
      - "Existing: EnforcementConfig / EnforcementPreset — unchanged"
    application:
      - "Existing: RunInput.enforcement_preset; OrchestratorBuilder::with_enforcement_preset"
      - "New (if (a)): a config-override path applied per run instead of at startup"
    infrastructure:
      - "None"
    api:
      - "Modified: cli --enforcement argument behaviour, or its removal"

  canonical_references:
    - module: ".pi/architecture/modules/enforcement.md"
    - adr:
        - ".pi/architecture/decisions/ADR-008-budget-enforcement.md"
    - last_sync: "2026-10-23"
    - evidence: "engine/.pi/architecture/modules/enforcement.md:23-26 — the flag is documented as threaded-and-not-read"
    - evidence: "cli/src/cli_boundary/dispatch.rs:220 (`enforcement: Option<String>`), :233 (`enforcement_preset: enforcement`), :981, :990"
    - evidence: "engine/src/orchestrator/application/dto/mod.rs:50,196 — the field, twice"
    - evidence: "engine/src/orchestrator/application/builder_impl.rs:30,196-197 — field and setter"
    - evidence: "grep: every other construction sets `enforcement_preset: None` (tui, mcp, actions, 20+ test fixtures) and no site reads it"
    - evidence: "cli/src/cli_boundary/orchestrator.rs:395 — `enforcement: Some(enforcement_config_from_engine(&engine_config.enforcement))`, i.e. from the config file"
    - evidence: "engine/src/execution_engine/application/parallel_executor.rs:80-84 — 'Entry points pass EnforcementConfig::standard() unless a preset was…'"

  acceptance_criteria:
    - "The flag either changes the resolved enforcement config for that run, or is refused with an actionable error, or no longer exists"
    - "No code path stores a preset that nothing reads"
    - "A test asserts the chosen behaviour; passing `--enforcement` in a test cannot pass silently as a no-op"
    - "The module doc's 'deliberately recorded' note is updated to match the chosen resolution"
    - "validate-ci.sh, validate-tests.sh, validate-architecture.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    **Recommendation: (b) reject loudly now, (a) later.** A per-run override is real work — the
    executor config is built once at startup, so the flag needs a path that replaces the config
    object per run, not a value threaded into a DTO. Until that exists, refusing the flag is the
    only option that does not mislead. Silent acceptance of a security setting is the failure mode
    worth spending a breaking change on.

    If (c) is chosen instead, remove all three surfaces together — the clap arg, `RunInput`, and
    `with_enforcement_preset` — because a DTO field that exists and is never read is how this
    happened in the first place, and leaving it invites the same wiring attempt.

    Do not implement (a) by mutating a global. The reason two concurrent runs do not share counters
    today is that the enforcer is per execution keyed by `dag_id`; a per-run config must follow the
    same shape, or the fix introduces cross-run interference that is far worse than the no-op.

    Whatever is chosen, the doc note at `enforcement.md:23` changes in the same MR. Two documents
    describing different realities is the defect this issue is about.

  file_changes:
    - "modify: cli/src/cli_boundary/dispatch.rs"
    - "modify: engine/src/orchestrator/application/dto/mod.rs (if (c))"
    - "modify: engine/src/orchestrator/application/builder_impl.rs (if (c))"
    - "modify: engine/.pi/architecture/modules/enforcement.md"
    - "create: cli/tests/enforcement_preset_flag.rs (or extend an existing CLI test)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---
# ISSUE-1156: `--enforcement strict` is accepted, threaded, and changes nothing

## What you are looking at

`rigorix run --enforcement <preset>` is offered as a way to override the enforcement preset for a
single run. It parses, it validates, it flows into the orchestrator builder — and then nothing reads
it. The enforcement config that actually governs the run comes from `.rigorix` (the configuration
file), resolved once at startup.

An operator who passes `--enforcement strict` believes the run is hardened. It is not. Nothing
reports the difference, including `rigorix doctor`, which reports the config-file preset as though it
were what was asked for.

This is not a case of a doc lying. The architecture records the gap honestly
(`engine/.pi/architecture/modules/enforcement.md:23-26`):

> **Still asymmetric, deliberately recorded:** the per-run `--enforcement` flag
> (`RunInput::enforcement_preset`) is threaded and not yet read — the executor config is built once at
> startup, so the flag cannot vary per run. Selecting a preset per run needs a config-override path,
> not a wiring fix.

The **interface** is what lies. That asymmetry is worse than a documented limitation, because the
person at risk reads the flag, not the module doc.

## Evidence, end to end

```
rigorix run --enforcement strict
  cli/src/cli_boundary/dispatch.rs:220   enforcement: Option<String>
                              :233       RunIntent { enforcement_preset: enforcement }
                              :981,:990  → handle_run(orch, intent, enforcement)
  engine/src/orchestrator/application/dto/mod.rs:50,196    pub enforcement_preset: Option<String>
  engine/src/orchestrator/application/builder_impl.rs:30   field
                                             :196-197  fn with_enforcement_preset(mut self, preset: String)
                                                       ↓
                                            NEVER READ   ← the defect ends here
```

Where the real value comes from:

```rust
// cli/src/cli_boundary/orchestrator.rs:395
enforcement: Some(enforcement_config_from_engine(&engine_config.enforcement)),
```

`engine_config` is the `.rigorix` configuration — the file. The flag never reaches this line.

Proof that the field is write-only: every other construction site passes `None`, and no site
dereferences it.

```bash
grep -rn 'enforcement_preset' . --include=*.rs
#   .pi-less constructions, all `None`:
#     mcp/src/execution_tools/infrastructure/engine_facade_impl.rs:323, :554
#     cli/src/tui/mod.rs:703
#     cli/src/tui/orchestrator_spawner.rs:152
#     actions/src/action_entrypoint/application/router_impl.rs:78
#     engine/src/orchestrator/application/builder_impl.rs:175, :335, :460
#     ~20 test fixtures
#   the only non-None assignments:
#     cli/src/cli_boundary/dispatch.rs:233
#     engine/src/orchestrator/application/builder_impl.rs:196-197 (the setter)
#   reads: none
```

The engine's own comment at `parallel_executor.rs:80-84` says the same:

> "Entry points pass `EnforcementConfig::standard()` unless a preset was…"

## The decision (already made — implement, do not re-litigate)

**Reject the flag loudly now; implement the per-run override as separate work.**

Reasoning: a per-run override is not a wiring fix. The executor config is built once at startup, so
the flag needs a path that replaces the config object *per run*. Until that path exists, refusing the
flag is the only option that does not mislead — and silently accepting a security setting is worth a
breaking change to stop. Two alternatives if you disagree with the ordering:

- **(c) delete the surface** — remove the clap arg, `RunInput.enforcement_preset`, and
  `with_enforcement_preset` together. A DTO field that exists and is never read is exactly how this
  happened; leaving it invites the same wiring attempt.
- **(a) implement now** — only if you can do it without mutating global state (see Pitfalls).

If you choose (a) or (c) rather than (b), say so in a comment before implementing, so the record
shows the decision changed.

## In Scope

- The chosen resolution for the flag: refuse with an actionable error / remove / implement
- `engine/.pi/architecture/modules/enforcement.md:23-26` updated to describe the new reality in the
  same MR
- A test that fails if the flag is a silent no-op again

## Out of Scope

- Preset definitions, limits, or the `[enforcement] preset` config-file setting (it works)
- Per-edge, per-tool, or per-node enforcement overrides
- The `doctor` command's other probes

## Affected Layers

### Domain
- `EnforcementConfig` / `EnforcementPreset` — unchanged

### Application
- `RunInput.enforcement_preset` and `OrchestratorBuilder::with_enforcement_preset` (remove or use)
- If (a): a per-run config-override path applied where the plan is dispatched

### Infrastructure
- None

### API
- The CLI's `--enforcement` argument: behaviour or removal

## Canonical References

| what | where |
|---|---|
| the honest note | `engine/.pi/architecture/modules/enforcement.md:23-26` |
| the flag | `cli/src/cli_boundary/dispatch.rs:220,233,981,990` |
| the DTO field | `engine/src/orchestrator/application/dto/mod.rs:50,196` |
| the builder | `engine/src/orchestrator/application/builder_impl.rs:30,196-197` |
| the real source | `cli/src/cli_boundary/orchestrator.rs:395` |
| the engine contract | `engine/src/execution_engine/application/parallel_executor.rs:80-84` |

## Acceptance Criteria

| # | Criterion | How it is checked |
|---|-----------|-------------------|
| 1 | Flag changes behaviour, is refused with an actionable message, or is gone | `validate-tests.sh` + a case that fails today |
| 2 | No stored-but-unread preset remains anywhere | `validate-architecture.sh` (grep-based check in the MR) |
| 3 | The error names the config file as the supported path (if (b)) | review of the message text |
| 4 | `enforcement.md:23` matches the resolution | `validate-canonical.sh` |
| 5 | Layer boundaries respected | `validate-architecture.sh` |
| 6 | No test asserts the no-op as correct | review |

## Implementation

> You are implementing a decision, not choosing one. If (b): the error message must name the config
> file, because the operator's immediate next question is "then how do I set it?".

1. Read `cli/src/cli_boundary/dispatch.rs:200-250` and `:960-1000` — the arg, the intent, the calls.
2. Read `engine/src/orchestrator/application/builder_impl.rs:20-40` and `:185-205`.
3. Read `cli/src/cli_boundary/orchestrator.rs:385-400` — where the real config is built.
4. Confirm the field is read nowhere: `grep -rn 'enforcement_preset' . --include=*.rs`.
5. Implement the chosen option.
6. Update `enforcement.md:23-26`.
7. Add the test (see below).

### The test that matters

A test that constructs a run with `--enforcement strict` and asserts *something about the resolved
config* — not that the value was stored. Asserting `intent.enforcement_preset == Some("strict")`
would pass today against the bug, which is why no test caught it.

## Pitfalls

- **Do not implement (a) by mutating a global.** Two concurrent runs do not share counters today
  because the enforcer is per execution, keyed by `dag_id`. A per-run config must follow that shape;
  a global override introduces cross-run interference that is far worse than the no-op.
- **Do not leave the DTO field behind** if choosing (c). The field existing is the invitation.
- **Do not update the doc in a follow-up.** Two documents describing different realities is the exact
  defect being fixed.
- **Do not silence the flag with a warning log.** A warning on stderr during a long run is not an
  interface; the operator already believed the flag worked.

## Why this is high

The flag is a security control that silently does nothing. There is no error, no log, and no
doctor probe that compares requested vs effective — so the only way to discover it is to read the
source of the module the operator was told not to need.
