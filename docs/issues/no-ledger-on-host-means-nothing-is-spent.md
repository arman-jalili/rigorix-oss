---
guardian_issue:
  id: "ISSUE-NO-LEDGER-ON-HOST-MEANS-NOTHING-IS-SPENT"
  title: "on a host with no consumption ledger every approval reads as unspent, and the absence is silent"
  epic: "authority-boundary"
  component: "Approval"
  module: "approval"
  status: planned
  priority: high
  tracking_issue: 1264
  dependencies:
    - "#1255 — the bidirectional check, whose no-ledger branch this revisits"
    - "#1241 — the anti-rollback marker"

  intent: |
    The ledger lives outside the repository, keyed by the run key's fingerprint
    (`$HOME/.rigorix/approval-consumption-<fingerprint>.json`). On a host that has never run — a
    fresh CI runner, a second machine, a container with a clean home — that file does not exist, and
    `reject_rolled_back` takes the documented branch:

    ```
    let pending = matches!(record.status, ApprovalStatus::Pending);
    if !pending && !self.ledger_present {
        return Ok(());            // no second witness: only the pending direction is enforced
    }
    ```

    So a store that says `Consumed` loads without complaint, a store with terminal records verifies,
    and **a store restored to `Pending` loads as Pending** — the replay #1241 exists to refuse. The
    reader is told nothing: no error, no warning, no field.

    #1255 chose this deliberately, on the assumption that "no ledger file" means "a deployment that
    never used one". The common case is the opposite: a deployment that *did* use one, on another
    host. Those two states are indistinguishable from inside the process, and the safe direction for a
    single-use token is to refuse, not to assume innocence.

  in_scope:
    - "A configured consumption ledger path whose file is absent, beside a store that holds terminal records, is a refusal naming the path and the records — not a silent pass"
    - "A store with no terminal records and no ledger is an empty case: allowed, named, exit 0"
    - "`doctor` reports the ledger's absence as a capability gap, with the path it looked for and the record count it could not check"
    - "Seeds: terminal record + absent ledger → refused naming path and node; absent ledger + no terminal records → allowed; ledger present → unchanged"

  out_of_scope:
    - "Copying or shipping the ledger between hosts — where it lives is the trust boundary (#1242)"
    - "The head-binding of the ledger, which ADR-020 D5 proposes"
    - "The reverse case (markers with no store record), which is harmless and already reported (#1253)"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: verify refuses an unverifiable terminal record"
    infrastructure:
      - "Modified: reject_rolled_back distinguishes 'no ledger configured' from 'ledger configured but absent'"

  citations:
    - file: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
      note: ":646 reject_rolled_back — the `!ledger_present` early return"
    - file: "engine/.pi/architecture/modules/approval.md"
      note: "#1255's stated rule: 'When no ledger file exists there is no second witness, so only the pending direction is enforced'"
    - file: "engine/src/execution_engine/application/factory.rs"
      note: "the ledger path and its key fingerprint — configured by default, so 'configured' is the normal case"

  verification:
    - "terminal record, ledger file removed: load and verify refuse, naming the path and the node"
    - "ledger removed and no terminal records: allowed, exit 0"
    - "ledger present: behaviour unchanged from #1261"
---

# ISSUE-NO-LEDGER-ON-HOST-MEANS-NOTHING-IS-SPENT

## The two states the code cannot tell apart

| state | what it means | safe default |
|---|---|---|
| a deployment that never used a ledger | nothing was ever spent here | proceed |
| a host whose ledger is elsewhere or gone | everything was spent, and the witness is missing | refuse |

Both present as `ledger_present: false`. #1255 chose the first reading. A single-use token should
assume the second, because the cost is asymmetric: a spurious refusal is an operator with a documented
way out (`doctor` names the path; re-approval is one command), while a false pass is a replayed
dispatch that nothing records.

## Why "a fresh runner" matters

The demos and CI start on hosts with a clean `$HOME`. Any workflow that reuses a checked-out store
without the ledger — a cache hit on the repository, a container image built with the store baked in —
is a workflow where every approval is unspent. The guard cannot see it, and the artifact cannot show
it afterwards.

## The required end state

A configured-but-absent ledger beside terminal records is a refusal, naming the path and the records it
could not check. Absence with nothing to check stays an allowed empty case. `doctor` reports what it
could not verify rather than reporting success.

## How it is proven

| seed | expected |
|---|---|
| terminal record, ledger removed | refused, naming path and node |
| no terminal records, no ledger | allowed, exit 0, names the path |
| ledger present | unchanged |
| the `!ledger_present` early return restored | the first seed passes silently — the seed fails |
