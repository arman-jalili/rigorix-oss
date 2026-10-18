---
guardian_issue:
  id: "ISSUE-CAPACITY-PROCESS-ATOMICITY"
  title: "capacity_ledger: 'single-execution atomicity' understates the in-process guarantee, and the cross-session case is untested"
  epic: "capacity ledger"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: medium
  dependencies:
    - "ADR-018 distributed capacity ledger"
    - "The dispatch seam (a861e2a8)"

  intent: |
    Correct the stated guarantee and test the case people will actually rely on. The docs say
    the in-process adapter provides "single-execution atomicity". The implementation gives
    something stronger: one Mutex over one LedgerState, and the composition root creates ONE
    such instance per process, shared by every execution that process runs. So concurrent
    executions in a single process DO contend on the same aggregate.

    That is an under-claim, which sounds harmless, but it is still a wrong boundary: a reader
    cannot tell whether two sessions in one process share a limit, and no test says so. The
    opposite misreading (that OSS is per-execution only) leads operators to deploy the
    enterprise service for a single-host setup that does not need it.

  in_scope:
    - "Restate the guarantee as single-process (atomic across executions sharing the ledger instance), reserving 'cross-session/cross-replica' for the HTTP adapter"
    - "State explicitly that two sessions in one process contend on the same aggregate"
    - "Add a test that runs two concurrent executions against one executor and asserts the aggregate holds"
    - "State the inverse limit plainly: nothing coordinates two separate processes without the enterprise service"
    - "Update ADR-018, the module doc, the adapter docs and the README claim ceiling consistently"

  out_of_scope:
    - "Making the in-process adapter coordinate across processes (that is the HTTP adapter's job)"
    - "Any functional change to the adapter"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: InProcessLedger doc comment (guarantee statement)"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md#guarantee-ceiling"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - evidence: "engine/src/capacity_ledger/infrastructure/in_process.rs — 'Single-execution atomicity via a single mutex over all state'"
    - evidence: "engine/src/execution_engine/application/service_impl.rs:545 — capacity: Option<CapacitySetup> is per-executor"
    - evidence: "engine/src/execution_engine/application/service_impl.rs:488 — sessions: HashMap<Uuid, ExecutionSession> (one executor, many sessions)"
    - evidence: "engine/tests/capacity_ledger_composition_e2e.rs — no test runs two concurrent execute_graph calls"

  acceptance_criteria:
    - "The guarantee is described as single-process atomicity across executions sharing the instance"
    - "The docs state that separate processes are not coordinated without the enterprise service"
    - "A test runs two concurrent executions through one executor against one low cap and asserts the aggregate is never exceeded"
    - "The module doc, ADR-018, the adapter doc comment and the README claim ceiling agree"
    - "No wording anywhere implies the in-process adapter is per-execution-only"

  validators:
    - ci
    - tests
    - canonical

  implementation_notes: |
    - This is mostly a documentation-accuracy issue, but the test is the point: an untested
      guarantee is a claim, not a boundary.
    - The 50-way concurrency test exercises the adapter directly. That proves the mutex, not
      the composition. The missing test is two executions through one executor.
    - Keep the guarantee ceiling honest in the other direction too: do not let this rewrite
      drift into implying cross-process safety.

  file_changes:
    - "modify: engine/src/capacity_ledger/infrastructure/in_process.rs (doc comment)"
    - "modify: engine/.pi/architecture/modules/capacity-ledger.md"
    - "modify: engine/.pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - "modify: README.md (claim ceiling, if it repeats the wording)"
    - "create: engine/tests/capacity_ledger_cross_session.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-PROCESS-ATOMICITY — the stated guarantee is narrower than the real one

## The defect

The docs say **"single-execution atomicity"**. The implementation provides **single-process**
atomicity across every execution that process runs:

| mechanism | evidence |
|---|---|
| one mutex over all state | `in_process.rs` — `state: Mutex<LedgerState>` |
| one seam per executor, shared by all sessions | `service_impl.rs:545` `capacity: Option<CapacitySetup>` + `:488` `sessions: HashMap<...>` |
| one instance per process | `CapacitySetup::from_env` is called once per composition root |

So two sessions in one process **do** contend on the same aggregate. The docs say otherwise,
which is an under-claim — and it leaves the property untested.

## Why an under-claim is worth fixing

- A reader cannot tell whether two sessions share a limit.
- The opposite misreading — "OSS is per-execution only" — leads an operator to deploy the
  enterprise service for a single-host setup that does not need it.
- "Cross-session atomicity requires the HTTP adapter" is then repeated in the adapter docs and
  the module doc, so the error propagates.

## Correct boundary

| deployment | guarantee |
|---|---|
| one process, N executions | **atomic across executions sharing the instance** |
| N processes / N replicas | not coordinated — requires the enterprise ledger service |

## Evidence that the case is untested

`grep` for two concurrent `execute_graph` calls against one executor → **none**. The existing
50-way test calls the adapter directly, which proves the mutex, not the composition.

## Acceptance Criteria

See front matter. The load-bearing one: **a test that runs two concurrent executions through
one executor and asserts the aggregate is never exceeded.**
