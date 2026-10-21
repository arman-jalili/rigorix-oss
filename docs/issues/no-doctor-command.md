---
guardian_issue:
  id: "ISSUE-NO-DOCTOR-COMMAND"
  title: "cli: no `rigorix doctor` — nothing can answer 'is enforcement actually armed in this repo?'"
  epic: "docs hygiene"
  component: "CLI"
  module: "cli-boundary"
  status: planned
  priority: medium
  dependencies:
    - "init scaffolds no enforcement configuration"

  intent: |
    Five seams decide whether the engine enforces anything: precondition, approval binding, sequence
    policy, capacity, anchor. All are armed through environment or files, all fail closed or fail open
    in ways that are documented only in source, and none can be inspected from the CLI. An operator
    has no supported way to ask the one question that matters: what is armed here?

  in_scope:
    - "A doctor command that reports each seam as armed / unarmed / requested-but-unsatisfied, with the reason"
    - "Non-zero exit when a seam was requested and could not be satisfied"
    - "Render the same seam list init uses (one source of truth)"

  out_of_scope:
    - "Automatically fixing configuration"
    - "Network reachability checks"
    - "The capacity exposure read (documented as belonging where the state lives)"

  affected_layers:
    domain:
      - "None"
    application:
      - "cli_boundary — new command; reads the existing *Setup::from_env results"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - evidence: "0 refs to 'doctor' in cli/src"
    - evidence: "arm state is produced by PreconditionSetup::from_env, ApprovalBindingSetup::from_env, SequencePolicySetup::from_env, CapacitySetup::from_env — each already returns Result<Option<_>>, i.e. the information exists but is discarded into engine construction"
    - evidence: "capacity/runbook: a fresh process cannot read exposure — that is why this reports arming, not state"

  acceptance_criteria:
    - "One command reports all five seams with a machine-readable and human-readable form"
    - "A requested-but-unsatisfiable seam exits non-zero and names the env var or file"
    - "An absent seam is reported as 'unarmed' explicitly, never omitted"
    - "The command is read-only: it starts no execution and creates no ledger"

  validators:
    - tests
    - operations

  implementation_notes: |
    The setups already encode the answer. `from_env` returns `Result<Option<Self>, _>` for each seam:
    `Ok(None)` = not requested, `Ok(Some)` = armed, `Err` = requested but unsatisfiable. The
    information is produced at startup and then consumed by the factory; doctor just has to surface
    it instead of discarding it.

    Report arming, not runtime state. A fresh process cannot see another process's capacity exposure,
    and a doctor that printed zeros would look authoritative and be wrong — the same reasoning already
    recorded in docs/runbook-capacity-ledger.md.

  file_changes:
    - "modify: cli/src/cli_boundary/cli.rs"
    - "modify: cli/src/cli_boundary/dispatch.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-NO-DOCTOR-COMMAND

## The five seams and where the answer already lives

| seam | setup | armed result |
|---|---|---|
| precondition | `PreconditionSetup::from_env` | `Ok(Some)` / `Ok(None)` / `Err` |
| approval binding | `ApprovalBindingSetup::from_env` | same |
| sequence policy | `SequencePolicySetup::from_env` | same |
| capacity | `CapacitySetup::from_env` | same (`ConfigInvalid` on malformed) |
| anchor | anchor mode + credentials | mode is readable |

`rigorix doctor` is not new plumbing. Every seam already computes the answer at startup; the only
change is to print it rather than only feeding it to a factory.

## Why this is the highest-value usability item

Every other usability finding in this set compounds into this one question. `init` scaffolds nothing,
`key` produces an unrelated value, and activation is environment-only — but with a doctor command all
three become visible in one place: the user sees every seam listed as unarmed while holding the
config `init` gave them.

Without it, the only way to learn what is armed is to read source, which means in practice nobody
learns, which means the gates are off where people believe they are on.
