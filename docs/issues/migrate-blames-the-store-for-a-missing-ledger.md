---
guardian_issue:
  id: "ISSUE-MIGRATE-BLAMES-THE-STORE-FOR-A-MISSING-LEDGER"
  title: "migration reports a missing or empty ledger as 'approval store unavailable … integrity failure … read failed: No such file or directory', with a non-zero exit"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: medium
  tracking_issue: 1256
  dependencies:
    - "#1247 — the explicit migration command"
    - "#1251 — the rendered migration outcomes"

  intent: |
    The upgrade notes send every operator to `rigorix approve ledger migrate`. On a host with no
    ledger — a fresh deployment, or one whose ledger was already migrated — that command reports:

    ```
    $ rigorix approve ledger migrate
    Error: approval store unavailable: /repo/.rigorix/approvals.json: Approval consumption ledger
           integrity failure at '/home/u/.rigorix/approval-consumption-<fp>.json': read failed:
           No such file or directory (os error 2)
    exit 1
    ```

    Three things are wrong with one sentence:

    * the **store** is reported unavailable while the **ledger** is the file that is missing;
    * a raw errno is the explanation for a state that is not an error at all — a host with no
      ledger has nothing to migrate, which is what the empty-file case already says
      (`file is empty — nothing to migrate`), also with exit 1;
    * the exit status tells a scripted upgrade that migration failed when nothing was required.

    A routine no-op that reports a broken store teaches operators to ignore the sentence, and the
    next line they ignore will be a real one.

  in_scope:
    - "No ledger at the path is a no-op: it says there is no ledger at that path and nothing to migrate, exit 0"
    - "An empty ledger file is the same no-op with the same exit status"
    - "A corrupt ledger, a MAC mismatch, an unsealed ledger, and a migration that would leave terminal records unprotected all remain failures"
    - "A ledger failure is reported as a ledger failure, not as an unavailable store"

  out_of_scope:
    - "Changing what migration does to a ledger it can read"
    - "The sealed/unsealed rules, which work"

  affected_layers:
    domain:
      - "Modified: migration distinguishes absent, empty, corrupt and unsealed rather than funnelling them through one integrity wrapper"
    application:
      - "Modified: the CLI's exit status for the no-op cases"
    infrastructure:
      - "None"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "measured: `rm <ledger> && rigorix approve ledger migrate` → 'approval store unavailable: … read failed: No such file or directory (os error 2)', exit 1"
    - evidence: "measured: `: > <ledger>` → '… file is empty — nothing to migrate', exit 1"
    - evidence: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs — migrate reads the ledger with std::fs::read_to_string and maps the io::Error into ledger_integrity"

  acceptance_criteria:
    - "No ledger: a sentence naming the path, exit 0"
    - "Empty ledger: a sentence saying nothing was migrated, exit 0"
    - "Unreadable ledger (permissions): a failure naming the ledger path, not the store"
    - "Corrupt JSON, bare map, MAC mismatch: failures as today, still naming the ledger"
    - "Seeds cover all five, including the exit statuses"

  validators:
    - ci
    - tests
    - docs

  implementation_notes: |
    - The wording to preserve is the one #1251 wrote: it says what was and was not compared. The
      same discipline applies to a no-op — say that nothing was there to migrate.
    - The wrapper that prefixes `approval store unavailable` is applied to every repository error;
      a ledger-only failure needs to escape it or carry its own subject.
    - Do not return success for a *corrupt* ledger to make the routine cases green. The
      distinction is absence versus damage, and only one of them is a no-op.

  file_changes:
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "modify: cli/src/cli_boundary/dispatch.rs"
    - "modify: cli/tests/doctor_cli.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-MIGRATE-BLAMES-THE-STORE-FOR-A-MISSING-LEDGER

## Measured

```
$ rm ~/.rigorix/approval-consumption-<fp>.json
$ rigorix approve ledger migrate
Error: approval store unavailable: /repo/.rigorix/approvals.json: Approval consumption ledger
       integrity failure at '…/approval-consumption-<fp>.json': read failed:
       No such file or directory (os error 2)                                   exit 1

$ : > ~/.rigorix/approval-consumption-<fp>.json
$ rigorix approve ledger migrate
Error: approval store unavailable: …: file is empty — nothing to migrate        exit 1
```

## Why a no-op must not be an error

The command is the documented upgrade step, so it is run on hosts where it has nothing to do.
Reporting a broken store and a non-zero exit for "there is no ledger here" is how an operator
learns that this command's failures do not mean anything — which is the state in which a real
failure is also ignored.

## The required end state

Absent and empty are no-ops with exit 0 and a sentence that names what was not found. Unreadable,
corrupt, unsealed and unprotected-terminal-record cases remain failures, and a ledger failure says
the ledger failed.

## How it is proven

| seed | expected |
|---|---|
| no ledger | "there is no ledger at `<path>`; nothing to migrate", exit 0 |
| empty ledger | "the ledger at `<path>` is empty; nothing to migrate", exit 0 |
| unreadable ledger | failure naming the ledger path |
| malformed ledger | failure naming the ledger path |
| MAC mismatch | failure naming the ledger path and the markers |
