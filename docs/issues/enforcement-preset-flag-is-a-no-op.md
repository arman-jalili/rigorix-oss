---
guardian_issue:
  id: "ISSUE-ENFORCEMENT-PRESET-FLAG-IS-A-NO-OP"
  title: "cli: --enforcement is a documented override that is threaded and never read — passing it changes nothing"
  epic: "docs hygiene"
  component: "CLI"
  module: "cli-boundary"
  status: planned
  priority: high
  dependencies: []

  intent: |
    `rigorix run --enforcement <preset>` is offered as an enforcement-preset override. The value is
    parsed, passed into RunInput::enforcement_preset, and never read by anything. A user who selects
    a stricter preset gets the default behaviour and no indication that the flag did nothing.

    A flag that silently does nothing is worse than an absent flag: it is a control the operator
    believes they have set.

  in_scope:
    - "Decide: implement per-run preset selection via a config-override path, or remove the flag"
    - "If implemented: the preset must reach the executor's config, which is built once at startup today"
    - "If removed: remove the flag, the DTO field and the builder method, so the surface stops promising it"
    - "Either way, the doc that currently records the limitation is updated"

  out_of_scope:
    - "Redesigning the executor config lifecycle (a per-run override is the smallest change)"
    - "Startup-level preset selection, which works today via environment/config"

  affected_layers:
    domain:
      - "None"
    application:
      - "orchestrator RunInput::enforcement_preset (threaded, unread)"
    infrastructure:
      - "cli_boundary: --enforcement flag and builder"
    api:
      - "CLI surface"

  canonical_references:
    - evidence: "cli/src/cli_boundary/cli.rs:138-140 — '/// Optional enforcement preset override.' + 'enforcement: Option<String>'"
    - evidence: "orchestrator/application/dto/mod.rs:50 and :196 — RunInput::enforcement_preset"
    - evidence: "orchestrator/application/builder_impl.rs:196 — with_enforcement_preset stores it; nothing consumes the stored value"
    - evidence: "every occurrence outside declarations is construction with None (grep: no read site)"
    - evidence: "engine/.pi/architecture/modules/enforcement.md:24 — recorded as 'threaded and not yet read'"

  acceptance_criteria:
    - "Passing --enforcement produces an observable change in the resolved enforcement config, or the flag is gone"
    - "A test asserts the resolved preset differs from the default when the flag is passed"
    - "If removed, no doc, DTO field or builder method still advertises per-run selection"
    - "The chosen behaviour is stated in the CLI help text truthfully"

  validators:
    - tests
    - canonical

  implementation_notes: |
    The blocker is real: the executor config is built once at startup, so a per-run preset needs a
    config-override path into the executor's factory. That is why the value was threaded rather than
    wired — but threading it without a consumer converts a visible gap into a silent one.

    Two honest outcomes, and the second is cheaper than it looks:

    - **implement** — pass an override through ParallelExecutionFactoryConfig and rebuild the
      enforcement slice per run (the preset is a small enum, not the whole config);
    - **remove** — delete the flag, the DTO field and the builder method. Nobody is using a no-op.

    What must not survive is the third option, which is the current state: the flag exists, is
    documented, is accepted, and is discarded.

  file_changes:
    - "modify: cli/src/cli_boundary/cli.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs"
    - "modify: orchestrator/application/dto/mod.rs (if removed)"
    - "modify: orchestrator/application/builder_impl.rs (if removed)"
    - "modify: engine/.pi/architecture/modules/enforcement.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-ENFORCEMENT-PRESET-FLAG-IS-A-NO-OP

## Verified

```rust
// cli/src/cli_boundary/cli.rs:138
/// Optional enforcement preset override.
#[arg(long)]
enforcement: Option<String>,
```

```rust
// orchestrator/application/dto/mod.rs:50
pub enforcement_preset: Option<String>,
```

`with_enforcement_preset` stores it (`builder_impl.rs:196`). Nothing reads the stored value — every
other occurrence in the tree is `enforcement_preset: None` in construction or a test.

## Why high

The failure is invisible from the outside. A run with `--enforcement strict` produces the same
output as a run without it, and nothing warns. The operator's mental model — "I tightened the
preset for this run" — is wrong, and the only way to discover it is to read the source.

Contrast with the effect-scope oracle: that one is also unwired by default, but it is *documented*
as unwired and the absence is visible if you look. This one is presented as working, in the help text,
in the arg docs, and in the DTO.

## What "done" looks like

Not "the field is read". Passing a stricter preset must change a decision: a tool call permitted
under the default is refused under the stricter preset, asserted end to end. A test that asserts
`config.preset == Some("strict")` proves the plumbing, not the effect — and plumbing that carries a
value into a field nobody consults is exactly what exists today.
