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

# ISSUE-1156: `--enforcement` is accepted and read by nothing

## Intent

The CLI promises an override; the engine reads the file. Make them agree.

## The path, end to end

```
rigorix run --enforcement strict
  cli/cli_boundary/dispatch.rs:220   enforcement: Option<String>
                              :233   RunIntent { enforcement_preset: enforcement }
  orchestrator/application/dto/mod.rs:50,196   pub enforcement_preset: Option<String>
  orchestrator/application/builder_impl.rs:30  field
                                          :196  fn with_enforcement_preset(..)
                                              ↓
                                    never read  ← the whole defect

actual enforcement:
  Configuration.enforcement  (the .rigorix file)
    → cli_boundary/orchestrator.rs:395 → ParallelExecutionFactoryConfig.enforcement
```

## In Scope

- One of: implement the override / refuse the flag / delete the surface
- Doc and DTO aligned in the same MR
- A test that fails if the flag is a no-op

## Out of Scope

- Redesigning presets; the config-file path (works)

## Affected Layers

### Domain
- `EnforcementConfig`/`EnforcementPreset` unchanged

### Application
- `RunInput.enforcement_preset`, `with_enforcement_preset`; a per-run override if (a)

### Infrastructure
- None

### API
- CLI flag behaviour or removal

## Canonical References

- **Module:** `.pi/architecture/modules/enforcement.md:23-26`
- **Evidence:** `dispatch.rs:220,233,981,990`; `dto/mod.rs:50,196`; `builder_impl.rs:30,196-197`; `orchestrator.rs:395`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Flag changes behaviour, is refused, or is gone | `validate-tests.sh` |
| 2 | No stored-but-unread preset remains | `validate-architecture.sh` |
| 3 | Test cannot pass with a silent no-op | `validate-tests.sh` |
| 4 | Module doc matches the resolution | `validate-canonical.sh` |
| 5 | Layer boundaries respected | `validate-architecture.sh` |

## Implementation

> **Agent:** the decision above is the deliverable. If (b), the error message must name the config
> file, because the operator's next question is "so how do I set it?".

### Steps

1. Read `dispatch.rs:210-245`, `builder_impl.rs:190-200`, `orchestrator.rs:390-400`
2. Confirm the field is read nowhere (`grep -rn 'enforcement_preset' .`)
3. Implement the chosen option
4. Update `enforcement.md:23`
5. Add/extend the CLI test

## Notes

- **Silent acceptance of a security setting is the defect.** The doc is honest; the interface is not.
- A per-run override must not mutate global state — the enforcer is per execution keyed by `dag_id`.
- Remove all three surfaces together if choosing (c); a leftover DTO field is how this recurs.
- Update the doc in the same MR — two documents describing different realities is the bug.

## Why high

An operator who passes `--enforcement strict` believes the run is hardened. It is not, and nothing
reports the difference — including the `doctor` command, which reports the config-file preset as if
it were what was asked for.
