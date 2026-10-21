---
guardian_issue:
  id: "ISSUE-INIT-SCAFFOLDS-NO-ENFORCEMENT"
  title: "cli: `rigorix init` scaffolds no enforcement configuration and names none of the env vars that arm it"
  epic: "docs hygiene"
  component: "CLI"
  module: "cli-boundary"
  status: planned
  priority: medium
  dependencies: []

  intent: |
    `init` creates `.rigorix/` and one `rigorix.toml` holding orchestrator, logging and LLM settings.
    It mentions no precondition config, no approval binding, no sequence policy, no capacity
    declaration, and none of the environment variables that arm them. A new user finishes `init` with
    a directory that looks configured and an engine in which every gate is unarmed.

  in_scope:
    - "Make `init` tell the operator what is armed (nothing) and what would arm it"
    - "Enumerate the seams in one place: precondition, approval binding, sequence policy, capacity, anchor"
    - "Handle the early return: today an existing .rigorix/ short-circuits, so older repos never see new guidance"

  out_of_scope:
    - "Writing config files that arm a gate by default (a scaffold that silently enables enforcement is worse)"
    - "Generating real secrets"
    - "A doctor command (filed separately)"

  affected_layers:
    domain:
      - "None"
    application:
      - "cli_boundary/dispatch.rs — cmd_init"
    infrastructure:
      - ".rigorix/ layout"
    api:
      - "None"

  canonical_references:
    - evidence: "cli/src/cli_boundary/dispatch.rs:95-124 — cmd_init writes only .rigorix/rigorix.toml (orchestrator, logging, llm)"
    - evidence: "cmd_init contains 0 mentions of hmac, precondition, capacity, approval or policy"
    - evidence: "cli/src/cli_boundary/dispatch.rs:97-99 — returns early when .rigorix/ exists"
    - evidence: "26 readers of RIGORIX_HMAC_KEY; none of them are mentioned by init"

  acceptance_criteria:
    - "After `init`, the operator can see which seams are armed and which are not, without reading source"
    - "The guidance names the env var for each seam and is generated from one list, not prose copies"
    - "A repo that already has .rigorix/ still receives the guidance (no silent early return for the advisory part)"
    - "init does not arm anything implicitly; a fresh `init` leaves every gate unarmed and says so"

  validators:
    - tests
    - ci

  implementation_notes: |
    The failure mode is not "init forgot a file" — it is that the output of `init` is
    indistinguishable from a configured workspace. A user who sees `.rigorix/rigorix.toml` reasonably
    concludes the tool is set up, and then runs with no gate armed and no indication.

    Prefer a single source of truth for the seam list (a const array of name + env var + what it
    enforces) that both init and the doctor command render. Two hand-maintained lists will diverge,
    which is the defect this issue is about.

  file_changes:
    - "modify: cli/src/cli_boundary/dispatch.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-INIT-SCAFFOLDS-NO-ENFORCEMENT

## What init writes

```rust
fn cmd_init() -> DispatchResult {
    let path = std::path::Path::new(".rigorix");
    if path.is_dir() {
        return DispatchResult::success(".rigorix/ already exists");   // early return
    }
    std::fs::create_dir_all(path.join("templates"))?;
    // .rigorix/rigorix.toml:
    //   [orchestrator] max_parallel_tasks, max_retries, default_timeout_secs
    //   [logging]      level, format
    //   [llm]          provider, model
}
```

That is the complete scaffold. Mentions of `hmac`, `precondition`, `capacity`, `approval`, `policy`: **0**.

## The gap

The engine has five seams that change its behaviour, all armed through environment or files that
`init` never mentions:

| seam | how it arms |
|---|---|
| precondition gate | `.rigorix/preconditions.toml` |
| approval binding | `RIGORIX_APPROVAL_BINDING` + HMAC key |
| sequence policy | `SequencePolicySetup::from_env` |
| capacity ledger | `.rigorix/capacity.toml` |
| anchor | anchor mode + credentials |

A fresh `init` leaves all five unarmed, produces a config file that configures none of them, and
prints "Initialized .rigorix/ directory". Nothing in that exchange distinguishes a configured
workspace from an unarmed one.

## The early return makes it permanent

`if path.is_dir() { return success(...) }` means a repo initialised by an older version never receives
any later guidance. Whatever `init` learns to say, existing repos will not hear it unless the advisory
part runs regardless of whether the directory exists.
