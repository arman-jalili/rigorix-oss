# rigorix-engine

**The core library. It contains every business rule, and it contains no CLI, no
GitHub Action and no server-specific code** — `rigorix-cli`, `rigorix-actions`,
`rigorix-mcp` and `rigorix-server` are all thin adapters over this crate.

If you are evaluating Rigorix, the two things worth knowing about this crate are
that **the enforcement path is deterministic Rust, not a model**, and that
**approval, sequence and consequence decisions all evaluate at one choke point**
immediately before a tool is dispatched.

```toml
[dependencies]
rigorix-engine = "1.10"
```

---

## Architecture

**Domain-Driven Design with bounded contexts as independently evolvable modules**
(Modular Monolith, [ADR-001](.pi/architecture/decisions/ADR-001-architecture-pattern.md)).
There are **35 modules** — 32 full Clean Architecture bounded contexts plus 3
cross-cutting modules (`common`, `observability`, and the `backend` integration
layer). Each bounded context has its own `domain/`, `application/` and
`infrastructure/` layers and its own frozen contracts, so a module can be
understood, tested and changed without holding the rest in your head.

```
rigorix-engine/src
├── planning/ ──────────── intent → plan → DAG
├── execution_engine/ ──── the dispatch loop and its single choke point
├── <governance>/ ──────── risk, permission, policy, approval, sequence,
│                         precondition, identity, budget, quality, scoring
└── audit/ ─────────────── signed envelopes, chain, anchor, projection
```

### Planning and compilation

| Module | What it does |
|---|---|
| `planning_pipeline` | Intent → classified template → extracted parameters → a reviewable plan |
| `planning` | The plan itself: typed steps, DAG construction, canonical hashing |
| `dag_engine` | Topological execution order, cycle detection, ready-queue scheduling |
| `template_system` / `template_generation` | TOML templates (`ADR-002`) and generating one from a natural-language intent |
| `plan_validation` | Plan-time gates, including operator step requirements and sequence rules **before the graph is sealed** |

### Execution

| Module | What it does |
|---|---|
| `execution_engine` | The dispatch loop. **The single choke point** every dispatch passes through |
| `orchestrator` | Composes the services into a runnable engine (`build_action_orchestrator`) |
| `tool_system` | The tool contract and concrete tools (`run_command`, `file_write`, `edit_file`, `git_*`, …) |
| `llm_step` / `code_generation` | Content generation — the only place the model participates |
| `repo_engine` / `code_graph` | Repository state and symbol relationships |
| `recovery_recipes` / `cancellation` | Failure recovery and cooperative, graceful-then-immediate cancellation |

### Governance — the enforcement surface

These are the modules that make the product's claims. Each is deterministic and
each has a decision record.

| Module | What it enforces |
|---|---|
| `risk_gating` | Operator-authored risk policy over each step (`ADR-007`) |
| `permission_enforcer` | Paths and capabilities the agent may touch |
| `policy_engine` | Declarative policy over plan and step data |
| `enforcement` | The runtime refusal surface; budgets via **RAII reservations** (`ADR-008`) |
| `budget_tracking` | Token and cost ceilings, reserved before the call and released after |
| `approval` | Human sign-off bound to a canonical `ExecutionIntent`, single-use and re-verified at dispatch (`ADR-011`) |
| `sequence_policy` | **Composed-action gating** over ordered step sequences (`ADR-013`) |
| `precondition` | **Consequence gating**: dispatch-time checks, argv or external determination service, with normative non-inheritance (`ADR-017`) |
| `identity` | Attributed, time-bound identity claims; OSS attests, the enterprise authorises (`ADR-012`) |
| `quality_gates` / `scored_evaluation` | Post-execution validation and pluggable rubric scoring over a Rigorix-defined protocol (`ADR-010`) |

### Evidence

| Module | What it records |
|---|---|
| `audit` | HMAC-signed envelopes, the hash-linked chain, the external anchor, and the authority-fact projection (`ADR-016`) |
| `state_persistence` | Crash-safe state via write-rename (`ADR-006`) |
| `event_system` | The execution event bus and its replay window (`ADR-005`) |

### Platform

`configuration` (multi-source config), `error_handling` (the typed taxonomy,
`ADR-009`), `observability` (structured tracing), `hooks`, `common`,
`failure_classification` and `failure_parser` (turning a failure into a
classified, actionable signal).

---

## The execution model

```
 intent ─► classify ─► template ─► parameters ─► DAG
                                                  │
                          ┌───────────────────────┘
                          ▼
                   PLAN-TIME GATES
                   operator requirements, sequence rules
                   (before the graph is sealed)
                          │
                          ▼
              ┌───────────────────────────────┐
              │  THE DISPATCH CHOKE POINT     │
              │  1. re-derive intent, compare │
              │     to the approval hash      │
              │  2. evaluate preconditions    │
              │     against live authority    │
              │  3. dispatch — or refuse      │
              └───────────────────────────────┘
                          │
                          ▼
             signed envelope (HMAC + chain + anchor)
```

The choke point is the design. Because approval re-verification, sequence prefix
gating and precondition evaluation all evaluate at **one** insertion point, there
is no alternate dispatch path to forget about — and a proofing contract test
asserts none exists (`ADR-011` §2).

**No LLM in the enforcement path.** The model generates content; the plan and the
operator's policy decide what is allowed to happen. This is a deliberate,
repeated decision (`ADR-007`, `ADR-011`, `ADR-013`, `ADR-017`).

---

## What it does not do

Honest boundaries, because a security reader should not have to discover them:

- **It is not a workflow engine.** The precondition primitive is *match a step, run
  a check, refuse on failure, record the outcome* — no loops, no conditionals, no
  policy DSL.
- **It has no domain semantics.** It does not know what "beneficiary eligible"
  means; identity resolution and authority live in your domain (`ADR-013/014/015`).
- **A precondition narrows a window, it does not close it.** There is still a gap
  between the check at T_check and the action at T_action; the downstream system
  must enforce at its own commit point (`ADR-017` §Honest boundary).
- **The local audit cache is tamper-evident, not tamper-proof.** It is not trusted
  for decisions; the external ledger tier exists for that (`ADR-016`).
- **Approval binding strength depends on the tool.** Deterministic tools are bound
  at the payload; `llm_generate` is bound at the input, and its output is verified
  *post hoc* (`ADR-011` §3).

---

## Development

```bash
cargo test -p rigorix-engine                       # unit + integration
cargo test -p rigorix-engine --lib                 # unit only
cargo clippy -p rigorix-engine -- -D warnings
bash .pi/scripts/validate-architecture.sh          # module layering
bash .pi/scripts/validate-canonical.sh             # @canonical references
```

Architecture lives with the crate:

- [`.pi/architecture/modules/`](.pi/architecture/modules/) — one document per
  bounded context, each with its `@canonical` reference
- [`.pi/architecture/decisions/`](.pi/architecture/decisions/) — ADR-001 through
  ADR-017. Start with **ADR-011** (approval binding) and **ADR-017** (consequence
  gating)
- [`docs/spec/`](../docs/spec/) — the published, normative specifications

The workspace root [`README.md`](../README.md) has the product-level framing, and
[`CHANGELOG.md`](../CHANGELOG.md) has the release history.

---

## License

Licensed under either of MIT ([LICENSE-MIT](../LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](../LICENSE-APACHE)), at your option.
