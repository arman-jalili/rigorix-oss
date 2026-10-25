# 04 — Architecture

**Status: informative.** This document describes one way to satisfy the
requirements. It creates no obligations.

---

## 1. Layers

```
┌──────────────────────────────────────────────────────────────┐
│ Surface        CLI · MCP · HTTP/JSON-RPC · CI adapter        │
│                all routes delegate to the same governed path │
├──────────────────────────────────────────────────────────────┤
│ Gate           approval binding → precondition →             │
│ (one module)   sequence + companion → capacity               │
├──────────────────────────────────────────────────────────────┤
│ Execution      plan → sealed DAG → dispatch loop → tools     │
├──────────────────────────────────────────────────────────────┤
│ Evidence       envelope → chain → anchor → verifier          │
├──────────────────────────────────────────────────────────────┤
│ Control plane  policy, approvals, ledger, console, reports   │
│ (optional)     never in the execution path                   │
└──────────────────────────────────────────────────────────────┘
```

The important architectural claim is the second row: one gate module, called by
every route. Multiple routes are acceptable. Multiple gate semantics are not.

## 2. Components and responsibilities

| Component | Responsibility | Must not do |
|---|---|---|
| Planner | turn intent into a plan | decide admissibility |
| DAG engine | order, validate, seal | execute side effects |
| Gate | decide permit/promote/refuse/halt | execute |
| Executor | dispatch tools, manage retries/cancellation | decide |
| Approval store | hold bound approvals | interpret policy |
| Precondition runner | run operator checks | interpret domain meaning |
| Sequence evaluator | evaluate ordered rules | fetch authority |
| Capacity service | reserve/commit/release | authorize |
| Evidence writer | sign, chain, anchor | judge legitimacy |
| Verifier | check evidence offline | trust the producer |

Each row has one job. When a component starts deciding outside its row, the
guarantees become unreviewable.

## 3. Deployment modes

| Mode | Runs | Guarantees available |
|---|---|---|
| Engine only | stateless engine, local evidence | Levels 1–3; Level 4 partial (chain + HMAC, no anchor) |
| Engine + verifier | engine plus published verifier packages | Levels 1–4 without the enterprise plane |
| Engine + control plane | engine plus enterprise services | Levels 1–5, subject to the store and configuration |

Mode determines the claim. The same binary in two modes does not carry the same
guarantees, and a claim that ignores the mode is invalid (EG-081).

## 4. Trust boundaries

Three boundaries must be drawn explicitly.

**Operator boundary.** Rules, preconditions, authority sources and approval policy
are operator-owned. They must not be writable through the agent's execution
interface (EG-025, EG-072).

**Process boundary.** A dispatched command runs as a subprocess. It must not inherit
control-plane secrets (EG-054). It should run under a distinct OS identity when the
deployment requires containment; the engine is not a sandbox.

**Evidence boundary.** The producer signs; a third party verifies. If the verifier
needs the producer online, the boundary has collapsed (EG-065).

## 5. Data flow of a refusal

```
step matches precondition
  → runner invokes check (argv or HTTP), outside the agent-writable workspace
  → check exits non-zero
  → gate returns Refuse
  → executor records failure, marks dependents Skipped
  → evidence writer emits finding with rule id, exit code,
    input digest, check digest, authority digest, timestamp
  → envelope signed, chained, optionally anchored
```

Nothing in that flow requires the agent to cooperate. That is the point.

## 6. Single-writer rule for decisions

Two components must never both decide the same step. The gate decides; the executor
obeys. Where an implementation has a fast path (a single-node API, a batch mode, a
resume path), it must call the gate, not reimplement it (EG-050).

## 7. What the architecture does not provide

- Isolation from a compromised host.
- Transactionality in downstream systems.
- Correctness of operator checks.
- Prevention of collusion between an approver and an agent.
- Protection against an operator who rewrites their own history and anchor keys.

Those are deployment and governance concerns. Stating them keeps the architectural
claim honest.

## 8. Reference implementation mapping

| Layer | Rigorix component |
|---|---|
| Surfaces | CLI/TUI, `rigorix-server` (`/rpc`, `/events`, `/mcp`), MCP stdio, GitHub Actions |
| Gate | `execution_engine/application/service_impl/gate.rs` (#1074) |
| Execution | planning pipeline, DAG engine, dispatch loop |
| Evidence | audit module, ADR-016 envelopes, anchor client |
| Control plane | enterprise: policy engine, audit ingest/query, ledger console, approvals |
