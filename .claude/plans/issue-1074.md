# Batch 5 — Issue #1074

**Branch:** `issue/1074`

## Defect
`execute_graph` → `run_dispatch_loop` ran approval binding, precondition,
sequence and capacity gates. `execute_node` → `execute_node_flow` ran **none**.
Same tool, same plan, two answers — invisible in the signature.

## Fix — one choke point
New `service_impl/gate.rs`:

| Piece | Responsibility |
|---|---|
| `gate_node(...)` | gate order once: approval → precondition → sequence (prefix + companion) → capacity. Returns `NodeGate::{Dispatch, Promote, HaltIntent, Refuse}` |
| `apply_gate(...)` | dispatch-loop bookkeeping for a verdict (record denial, release dependents, pause) |
| `execute_node_flow` | calls `gate_node` before dispatching; `Refuse` → failed node result, `Promote`/`HaltIntent` → `approval_required` (no session to pause) |

Both `run_dispatch_loop` fill loops now call `apply_gate`, deleting ~360 lines of
duplicated gate + bookkeeping. Verified by the full 2225-test suite.

## Test
`gate::tests::the_shared_gate_sequence_refuses_a_step_over_the_capacity_cap` —
builds the executor with a real `CapacitySetup` fixture and asserts the shared
gate returns `capacity_denied` (the arm `execute_node` never reached).
