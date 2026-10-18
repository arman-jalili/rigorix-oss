---
guardian_issue:
  id: "ISSUE-FAILURE-CLASS-GUARD-SUITE"
  title: "spike: catalogue the failure classes that escaped gates and reviews, and build one guard per class"
  epic: "self-owned vulnerability checks"
  component: "Guards"
  module: "governance"
  status: planned
  priority: critical
  dependencies:
    - "#1067, #1068, #1069, #1070, #1071, #1072, #1073, #1074 — the round-3 series"
    - "ISSUE-SELF-OWNED-VULN-CHECKS — the guard registry this extends"

  intent: |
    Round 3 fixed nine defects, several of them serious, and **every gate and every review passed
    them**. That is the finding: not that the bugs existed, but that nothing in the pipeline was
    capable of seeing them. Each one is an instance of a small number of *classes*, and each class
    is mechanically detectable.

    This spike produces the class catalogue **with evidence from this repository**, and then
    delivers a working guard per class — registered in `.pi/guards.tsv`, gating, with a seeded
    violation that proves the guard fires.

    A guard that cannot demonstrate it fires is a comment with a shebang.

  in_scope:
    - "Catalogue each class with at least one concrete instance and its file/line"
    - "One guard per class, registered in .pi/guards.tsv, tier=gating"
    - "A seeded-violation test per guard: inject the defect, assert non-zero exit, revert"
    - "A `clippy.toml` with disallowed types/methods for process spawning (see below)"
    - "A manifest of decision-bearing config fields, consumed by the reachability guard"
    - "Fail the build if a guard exists but is unregistered (already true) and if a class in the catalogue has no guard"

  out_of_scope:
    - "Runtime/sandbox isolation of subprocesses (deployment concern)"
    - "Re-litigating any individual round-3 fix"
    - "Guards for classes with no instance in this repo (record them, do not build speculatively)"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: .pi/scripts/ (new guards), .pi/guards.tsv, clippy.toml"
      - "Modified: engine/Cargo.toml (lint wiring) if disallowed-types needs it"
    api:
      - "None"

  canonical_references:
    - registry: ".pi/guards.tsv"
    - existing: "engine/.pi/scripts/validate-code-graph-contracts.sh (check_impl / check_contains precedent)"
    - existing: "ISSUE-SELF-OWNED-VULN-CHECKS — the 'a guard must be registered' rule"
    - precedent_for_forbid: "rigorix-enterprise core/.pi/scripts/ci/check_capacity-ledger_contracts.sh (forbid() — pattern must be ABSENT)"

  acceptance_criteria:
    - "The catalogue names each class, one instance per class, and the class owner"
    - "Every catalogued class has a gating guard in .pi/guards.tsv"
    - "Each guard fails on a seeded violation and passes when reverted (recorded in the PR)"
    - "clippy.toml bans process-spawn types outside the approved module"
    - "The decision-field manifest is non-empty and the guard fails when a field is unreferenced"
    - "No guard is advisory unless the reason is recorded in the registry"
    - "The suite runs in the existing ci.yml audit job within a reasonable budget"

  validators:
    - ci
    - security
    - architecture
    - canonical

  implementation_notes: |
    Prefer moving an invariant into the type over writing a guard. A newtype that makes the
    wrong state unrepresentable (e.g. a validated command the executor alone accepts) cannot be
    bypassed; a grep can. Guards are for the residue the type system cannot express.

    For each guard, the question is "does it fail on the parent commit?" — run it against HEAD^
    after seeding the defect. That is the same red-green discipline the issue template asks of
    tests, applied to the guard itself.

    Keep the registry honest: a guard whose class has no instances here is a guess, and guesses
    rot. Record the class, do not build the guard.

  file_changes:
    - "modify: .pi/guards.tsv"
    - "create: clippy.toml"
    - "create: .pi/scripts/validate-exec-sink-confinement.sh"
    - "create: .pi/scripts/validate-decision-fields.sh"
    - "create: .pi/scripts/validate-requested-implies-armed.sh"
    - "create: .pi/scripts/validate-control-is-wired.sh"
    - "create: clippy.toml / decision-fields.toml manifests"
    - "modify: .github/workflows/ci.yml"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-FAILURE-CLASS-GUARD-SUITE

## The finding

Nine defects shipped past `ci`, `security`, `architecture`, `canonical`, `panic-sites`, clippy,
review, and a green suite — and several were serious: a command policy that could be bypassed by
a shell separator, the engine's own signing key handed to every spawned command, a public API
that skipped every consequence gate.

They are not nine unrelated mistakes. They are **nine instances of eight classes**, and every
class is mechanically checkable.

## The catalogue

### C1 — Unguarded sibling path

The same effect is reachable by a route that lacks the control.

| instance | evidence |
|---|---|
| `exec_run_command` had **no allowlist at all** — a second `sh -c` path | `service_impl/tools.rs` (#1067) |
| `execute_node` skipped approval, precondition, sequence, capacity | `service_impl.rs:882` (#1074) |

*Guard:* **sink confinement.** `clippy.toml` `disallowed-types`/`disallowed-methods` for
`std::process::Command` and `tokio::process::Command`, allowed only inside the approved module.
Any new spawn site then fails to compile unless it adopts the policy. This is the highest-leverage
change in the ticket.

### C2 — The policy examines a different string than the executor

| instance | evidence |
|---|---|
| prefix allowlist over a string handed to `sh -c` | `run_command_tool.rs:69` (#1067) |
| `check_bash` fed the serialized **JSON intent**, and `classify` takes `tokens[0]` | `tools.rs:58` (#1067) |

*Guard:* **type-level.** `ValidatedCommand` newtype; the executor accepts only it. Then the
compiler proves the validated bytes are the executed bytes. A grep guard is the fallback if a
newtype is too invasive: assert the only `.arg(raw_command)` site is the module that validates.

### C3 — Ambient authority leaks into the child

| instance | evidence |
|---|---|
| child inherited `RIGORIX_HMAC_KEY`; no `env_clear` anywhere | `run_command_tool.rs:125` (#1068) |

*Guard:* every spawn routes through one env-clearing helper; plus the allowlist check the
enterprise already has — no name on a child-env allowlist may match
`SECRET|KEY|PASSWORD|TOKEN|DSN|CREDENTIAL|_URL`. (That enterprise test is the pattern to copy.)

### C4 — The claim exceeds the evidence

| instance | evidence |
|---|---|
| `HistoryIntegrity::Anchored` derived from **mode**, not head | `envelope_factory_impl.rs:129` (#1071) |
| `verify_authority_fact_bundle` proved consistency, claimed provenance | `projection.rs` (#1073) |
| "companion step" enforced presence, not prerequisite | `companion_impl.rs` (#1069) |

*Guard:* **type-state.** An assertive label must carry its evidence (`Anchored(head)`, not
`Anchored`). Where that is impossible, require named outcomes: no `pub fn verify*` in `audit/`
may return `bool` or `()`.

### C5 — Configured, echoed, never consulted

| instance | evidence |
|---|---|
| `window_seconds` required, constrained, echoed — in no decision | (#1051, and enterprise #261) |
| `approval_repo_path` `None` in every entry point | (#1072) |

*Guard:* **decision-field reachability.** A manifest lists decision-bearing config fields; each
must occur in at least one non-DTO, non-Display, non-test file. Otherwise it is decoration.

### C6 — A requested guarantee silently degrades

| instance | evidence |
|---|---|
| `RIGORIX_APPROVAL_BINDING=1` with no key → warn, serve unbound | `factory.rs:217` (#1070) |

*Guard:* no `*_from_env` / `*Setup` may return a bare `Option`. `Result<Option<_>, _>` only, so
"not requested" and "requested but unsatisfiable" stop sharing a value.

### C7 — Nothing is wired

| instance | evidence |
|---|---|
| the capacity ledger was fully implemented, unit-tested, and referenced by nothing | fixed in `a861e2a8` |

*Guard:* **reachability.** Every `pub struct *Impl` in a bounded context needs at least one
non-test reference from another module. Zero refs → the control does not exist, whatever the
module doc says.

### C8 — The test encodes the defect

| instance | evidence |
|---|---|
| `expired_outstanding_stops_counting` asserted the bug verbatim; a green suite defended it | enterprise, #260 |

*Guard:* **red-green gate.** Every test added or changed in a PR must **fail** on the parent
commit. Buildable: check out `HEAD^` in a worktree, run the changed tests, require non-zero exit.
This is the same falsifier discipline the round-3 work did by hand, automated.

## Do not build the wrong thing

The temptation is to write a linter that greps for the nine specific bugs. That produces nine
regressions tests for nine fixed bugs and misses the tenth.

The classes above are expressed as **properties of the codebase** (every sink is confined, every
declared field is consulted, every claim carries its evidence), not as patterns of the known
defects. That is what makes them survive the next variant.

## Prefer the type over the guard

Where an invariant can be made unrepresentable, do that and skip the guard: `Anchored(head)`
cannot be constructed without a head, so no guard is needed. Guards are for the residue —
"every spawn goes through one function" is not a type, and "every new test fails on the parent"
is not a type either.

## Acceptance

See front matter. Load-bearing: **each guard must be shown to fire.** A guard added without a
seeded-violation test is indistinguishable from a guard that always passes — which is exactly the
failure mode this issue exists to end.
