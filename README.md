# Rigorix

<!-- Badges: every package below is verified published (checked 2026-10-15).
     The verifier badges are the ecosystem signal — the verifier is the artifact a
     third party actually runs, in their own language. -->
[![rigorix-verifier on crates.io](https://img.shields.io/crates/v/rigorix-verifier?label=rigorix-verifier%20%C2%B7%20crates.io)](https://crates.io/crates/rigorix-verifier)
[![rigorix-verifier on npm](https://img.shields.io/npm/v/rigorix-verifier?label=rigorix-verifier%20%C2%B7%20npm)](https://www.npmjs.com/package/rigorix-verifier)
[![rigorix-verifier on PyPI](https://img.shields.io/pypi/v/rigorix-verifier?label=rigorix-verifier%20%C2%B7%20PyPI)](https://pypi.org/project/rigorix-verifier/)
[![CI](https://github.com/arman-jalili/rigorix-oss/actions/workflows/ci.yml/badge.svg)](https://github.com/arman-jalili/rigorix-oss/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green)](LICENSE-MIT)

### Stop agents from turning individually-permitted actions into outcomes nobody approved.

**Rigorix is the governance layer underneath your agents.** It is for the teams that
have to let autonomous agents act — money-out flows, CI/CD, clinical and other
regulated pipelines — and must still be able to say, and prove, what those agents
were permitted to do.

Coding agents can now write, edit and ship software. The question organisations hit
is not *can they?* — it is *what did the agent do, who approved it, and what was it
allowed to touch?* Conversation history cannot answer that. An API gateway cannot
either: it governs what flows *into* your model, not what the agent does in your
repository, your shell, your CI.

Rigorix compiles a natural-language task into a reviewable plan, executes it inside
policy, permission and budget boundaries, and records every step in a signed audit
envelope. When an agent reaches for something risky, Rigorix does not warn — it
refuses, or gates the step for human approval.

> **Not a faster autocomplete.** Rigorix is not an agent you chat with; it is the
> layer the agent runs inside. `Claude Code`, `Cursor`, `Copilot`, `Aider` and
> `Codex` all keep their own loop and plug into it over MCP.

---

## Watch it stop

The fastest way to understand Rigorix is the [two-minute demo](https://github.com/arman-jalili/payments-demo): a coding agent fixes a real double-charge bug in a payments webhook — then its plan drifts toward a file it wasn't cleared to touch (`src/auth.ts`, signed off by a human). The step is marked `requires_approval = true`. Execution stops. The file is not touched. Until a human says yes.

Not a warning. A stop. One command, no API keys, fully local:

```bash
git clone https://github.com/arman-jalili/payments-demo
cd payments-demo
npm install && node .rigorix/run-demo.mjs
```

<!-- Visual: a GIF/video of this refusal belongs here. Deliberately not committed
     yet — see docs/assets/README.md for the required asset and how to record it.
     A broken <img> would be worse than none. -->

---

## Works with the agents you already run

Rigorix isn't a separate agent — it's a governance layer underneath yours. The [rigorix-mcp gateway](mcp/README.md) plugs Rigorix into any MCP-compatible coding assistant (Claude Code, Cursor, Aider) as a standard MCP server, exposing **15 built-in tools**:

- **Execution (6)** — `rigorix_execute`, `rigorix_run`, `rigorix_plan`, `rigorix_validate_plan`, `rigorix_check_enforcement`, `rigorix_approve_execution`
- **Templates (4)** — `rigorix_list_templates`, `rigorix_get_template`, `rigorix_create_template`, `rigorix_validate_template`
- **Audit (4)** — `rigorix_read_audit`, `rigorix_list_audits`, `rigorix_audit_summary`, and `rigorix_audit_authority_facts` (read-only — the gateway never modifies audit data)
- **Usage guide (1)** — `rigorix_get_usage_guide`, a self-documenting tool list and workflow patterns

When an identity provider is configured (`RIGORIX_IDP_ISSUER` + `RIGORIX_IDP_CLIENT_ID`), **3 auth tools** are added — `rigorix_auth_login`, `rigorix_auth_status`, `rigorix_auth_logout` — for **18** in total. They are absent from `tools/list` otherwise, rather than present and failing.

Your agent keeps its own loop; everything it does through Rigorix is planned, gated, and recorded.

```bash
cargo install rigorix-mcp
```

Add to `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "rigorix-mcp": {
      "command": "rigorix-mcp",
      "args": []
    }
  }
}
```

Then let the agent drive — plan, validate, execute, and audit from inside the conversation. Full docs: [mcp/README.md](mcp/README.md).

---

## The problem

Every agent loop today works the same way: an LLM decides what to do, does it, checks the result, and loops. That loop is powerful — and unstructured. There's no distinction between planning and execution. There's no audit trail beyond conversation history. There's no way to say *"execute this plan, but only if it stays within these boundaries."*

This works fine when a human is watching every step. It breaks down when you want to:

- **Run in CI/CD** — without a human approving every tool call
- **Audit what happened** — when conversation history isn't enough for compliance
- **Enforce policies** — "deny any change that touches the auth module" or "flag diffs that modify payment processing"
- **Budget costs** — cap LLM spending per run so a runaway agent doesn't burn your API key

Rigorix is opinionated: it intentionally gives up some **flexibility** in exchange for **repeatability, governance, and deterministic execution.**

The core idea: instead of an LLM deciding what to do at each step, the intent is compiled into a DAG first — a deterministic, reviewable plan. The DAG says: *read these files, generate this patch, run these tests, verify these conditions.* The LLM fills in the content; the DAG controls the flow. This is the same pattern that made build systems (Make, Bazel) and data pipelines (Airflow, Dagster) reliable: separate *what* from *how*, validate the plan before running it, and record every execution.

**Bounded autonomy:** every execution is constrained by configurable risk policies, permission rules, execution budgets, and quality gates. The model is intentionally restrictive: the LLM decides what to generate within the execution graph; Rigorix determines what is allowed to happen.

```
 Natural language task
        ↓
 Classifier — maps intent to a template
        ↓
 Template — defines the execution structure
        ↓
 Parameters — extracted from the task
        ↓
 DAG — deterministic execution graph
        ↓
 Execution — tools, retry, recovery
        ↓
 Validation — quality gates, policies
        ↓
 Audit — signed, timestamped record
```

---

## How Rigorix compares

Rigorix is a governance layer, so its neighbours are the other governance layers —
not the coding agents it sits underneath. `Claude Code`, `Cursor`, `Copilot`,
`Aider` and `Codex` are **governed by** Rigorix; comparing them to it would be a
category error.

**Every competitor cell below was read from the product's own site or repository
on 2026-10-15.** Where a claim could not be established from the source, the cell
says **not established** rather than inferring a weakness. Competitors ship
weekly: treat this table as dated, not durable.

| | **Rigorix** | **Cordum** | **Mault** | **APERION** | **microsoft/agent-governance-toolkit** |
|---|---|---|---|---|---|
| **What it is** | governance layer under any MCP agent | "The Action Firewall for AI Agents" | "Governance for Coding Agents" | "Runtime AI Governance for Enterprise Agents" | "AI Agent Governance Toolkit" |
| **Enforcement point** | compiled plan + per-step policy / permission / budget gate | pre-tool gate; policy, approval and redacted audit evidence before execution | deterministic rules "from keystroke to merge" | "in the path between your agents and every model and tool" | policy enforcement, zero-trust identity, execution sandboxing |
| **Deterministic, no LLM in the enforcement path** | yes — the enforcement path is Rust, not a model | not established | **yes** — "No LLM in the enforcement path" | not established | not established |
| **Signed record** | HMAC envelopes + chain anchor (ADR-016) | "redacted audit evidence" | "a signed record of every action"; "Signed, verifiable offline" | "a record bound to the human who owns it" | not established |
| **Verifier you can run yourself** | **yes** — the open-source `rigorix-verifier` 0.5.0, published on crates.io, npm and PyPI, and implemented in Go and Java | not established | not established | not established | not established |
| **Verification method** | **re-derivation from the anchored chain**, not trusting the producer's signature (spec §4.3.1) | not established | not established | not established | not established |
| **Stated claim ceiling** | **yes** — the artifact never asserts present standing (NC-4) | not established | not established | not established | not established |
| **Licence** | MIT / Apache-2.0 | source-available core | proprietary (commercial) | proprietary | MIT |
| **Repository signal** | — | cordum.io | mault.ai | aperion.ai; `AperionAI/shield` 11 stars, licence **conflicting** (repo description says open-source, README badge says Proprietary) | 6,430 stars, MIT, "covers 10/10 OWASP Agentic Top 10", pushed 2026-10-15 |

Sources, all read 2026-10-15: `cordum.io`, `mault.ai`, `aperion.ai`,
`gh api repos/AperionAI/shield`, `gh api repos/microsoft/agent-governance-toolkit`.
Rigorix's own cells are traced to: the published `rigorix-verifier` packages
(crates.io / npm / PyPI),
`docs/spec/consequence-gating-v1.md` §4.3.1 and NC-4, `ADR-016`, `ADR-017`.

### What this table is not

It is **not a benchmark**, and **no independent comparative evaluation has been
performed** — every cell is a reading of vendor-published material, and the
Rigorix column is a reading of our own artifacts. It establishes what each product
publicly says about itself on these dimensions; it does not establish relative
quality, and it should not be cited as if it did (L1/L2/L3 claim ceilings,
`docs/spec/README.md`). If a cell here is wrong, the correction is a source and a
commit — open an issue.

The distinction that matters, and the one we do stand behind: **CI/CD answers "did
the agent break the rules" — Rigorix answers "could the agent break them."** One
layer reports; the other refuses.

---

## Consequence gating: a refusal you can prove (v1.10.0)

The problem it solves: an agent's individual actions can each be permitted while
the **sequence** is not. Rigorix evaluates the consequence before the action, and
records both the decision and the evidence for it.

**Technical.** Before a step runs, a precondition is evaluated *outside the
agent-writable workspace* — an operator-supplied check program, or an external
determination service whose request and response bytes are bound into the record as
digests. A refusal is recorded in the signed envelope, not merely logged: for each
fact the record carries what was established, when, under which scope, and which
record established it. The envelope chain is hash-linked and anchored (ADR-016), and
an **authority-fact projection** exports that history for a third party to check.

The part that matters is *how* it is checked: by **re-derivation**, not by trusting
a signature. The verifier replays the derivation against the anchored chain and
compares the facts (spec §4.3.1). A signature proves only *who said it*;
re-derivation proves *what the history actually supports* — it catches a signer
asserting facts the chain does not back. Absence is explicit: a subject with no
record yields a `NOT_ESTABLISHED` fact, and the artifact **never asserts present
standing** (NC-4). Note the ceiling: this is engineering verification (L1), not
protocol validation (L2) and not independent evidence (L3) — see
[docs/spec/README.md](docs/spec/README.md).

**Business.** This is the question an auditor or regulator actually asks. When an
agent is stopped — or not stopped — you have to show what it was permitted to do,
on what evidence, and that the answer can be checked by someone who does not trust
you. Because the verifier is open source and published on **crates.io, npm and
PyPI**, and implemented in **Go** and **Java** as well, the reviewer runs the check
on their own machine, in their own language, with no round-trip to the vendor. The
risk it removes is the unauditable refusal — the failure mode where nobody can
reconstruct, months later, whether the control even worked.

```bash
# Export a bundle from the signed history…
rigorix audit projection --subject authority:beneficiary-eligible -o bundle.json
# …and verify it offline, by re-derivation, with no engine and no key.
rigorix audit verify-projection bundle.json
```

---

## Templates

Templates encode repeatable engineering workflows. Instead of asking the model to rediscover how to perform a common task each time, Rigorix selects an appropriate template, extracts parameters, builds an execution graph, and lets the LLM focus on generating code within that structure.

Repeatable means the same intent produces the same execution structure under the same templates and policies. The generated code may differ, but the workflow, validation steps, and governance remain consistent.

A template defines:

- **What** files to read and what to generate from them
- **Which** commands to run for verification (type-check, test, lint)
- **How** to handle dependencies between steps
- **Where** the output goes (new files, patches, test results)
- **Which** steps are gated — `requires_approval = true` pauses the run until a human decides

Here is a minimal template — it reads a file, runs a regex filter, and writes the result:

```yaml
name: extract-function-docs
description: Extract JSDoc comments from a TypeScript file
parameters:
  - name: file_path
    type: string
    description: Path to the TypeScript file
nodes:
  - id: read_file
    action: file_read
    params:
      path: "{{ file_path }}"
  - id: extract_docs
    action: llm_generate
    depends_on: [read_file]
    params:
      prompt: >-
        Extract all JSDoc comments from the file below.
        Return them as a markdown list.
      input: "{{ read_file.output }}"
  - id: write_output
    action: file_write
    depends_on: [extract_docs]
    params:
      path: "docs/{{ file_path | basename }}.md"
      content: "{{ extract_docs.output }}"
```

When a user runs `rigorix plan "Extract docs from src/api.ts"`, Rigorix classifies the intent, maps it to this template, prompts the LLM to fill `file_path`, and builds the 3-node DAG. **The LLM generates the doc content; the template controls the flow.**

When no existing template matches the intent — or confidence is low — Rigorix prompts the LLM to generate a new template dynamically. Generated templates can be cached and reused, reducing the need to regenerate common workflows.

Rigorix currently supports Rust, TypeScript, and Python as target codebases. TypeScript is the most mature integration today; the others are functional but earlier-stage.

---

## Quickstart

### Install

```bash
# From source
cargo install --git https://github.com/arman-jalili/rigorix-oss rigorix-cli

# Or build locally
git clone https://github.com/arman-jalili/rigorix-oss
cd rigorix-oss && cargo build --release -p rigorix-cli
./target/release/rigorix-cli --help
```

### Set your API key

```bash
export RIGORIX__LLM__API_KEY="sk-ant-..."   # Anthropic
# or: export ANTHROPIC_API_KEY="sk-ant-..."
```

### Initialize a project

```bash
cd my-project
rigorix init
```

### Plan before running (recommended)

```bash
rigorix plan "Add a new endpoint to the API"   # Review the DAG, then:
rigorix run "Add a new endpoint to the API"
```

Or plan and confirm in one flow:

```bash
rigorix plan "Add error handling to the parser"
# Shows plan, then prompts:
# Run this plan now? [y/N]: y
```

### Run your first task

```bash
rigorix run "Explain how the main module works"
```

Rigorix will: classify the intent → extract parameters → generate a DAG → execute nodes (file reads, edits, bash commands) → validate results.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                   User (Developer)                           │
│            (CLI / TUI / GitHub Action)                       │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                     Planning Phase                           │
│                                                              │
│  Intent → Classify → Extract → Generate TaskGraph → Validate │
│                  ↕ (low-confidence fallback)                 │
│        Template System + LLM Template Generator              │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                     Execution Phase                          │
│                                                              │
│  DAG Engine (topo sort) → ParallelExecutor (tokio JoinSet)   │
│       → Tool System (file/git/command/LSP)                   │
│       → Retry/Recovery/Fallback                               │
│       → Cancellation (graceful/immediate)                    │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                  Observability & Persistence                  │
│                                                              │
│  Event Bus → State Persistence → Audit (HMAC-signed)         │
│         + Prometheus Metrics + Tracing                       │
└─────────────────────────────────────────────────────────────┘
```

## Repository Structure

```
rigorix-oss/
├── engine/              # Core library — all business logic
│   ├── src/             # Core engine (planning, execution, tools, governance)
│   └── .pi/             # Architecture docs, ADRs, diagrams
├── cli/                 # CLI binary — thin wrapper over engine
│   ├── src/cli_boundary/# Flag-based CLI (Clap, dispatch, config)
│   ├── src/tui/         # Interactive TUI (ratatui)
│   └── .pi/             # Architecture docs
├── mcp/                 # MCP gateway server (rigorix-mcp) — bridges
│   │                    #   Claude Code / Cursor / Aider to the engine
│   ├── src/             # 8 bounded-context modules
│   └── .pi/             # Architecture docs
├── server/              # OSS reference execution host (rigorix-server) —
│   │                    #   JSON-RPC 2.0 + SSE over the frozen rigorix.* API
│   ├── src/             # transport, catalog, backend seam, version
│   └── .pi/             # Architecture docs
├── actions/             # GitHub Action — thin adapter over engine
│   ├── src/             # 9 bounded-context modules
│   └── .pi/             # Architecture docs
├── Cargo.toml           # Workspace root
└── .pi/                 # Root-level architecture docs, prompts, scripts
```

## Development

### Prerequisites

- Rust 2024 edition (stable toolchain)
- LLM API key (set `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`)

### CI as Continuous Verification

Rigorix treats CI as continuous verification rather than compilation and testing. Beyond formatting, linting, unit tests, and security scanning, every architectural capability is validated through proofing scripts that verify contracts, architecture readiness, documentation consistency, policy enforcement, and execution guarantees.

```
Rigorix runs its checks two ways, and both numbers below are derivable rather than
estimated — re-derive them with `bash .pi/scripts/local-ci.sh --list` and
`grep -c '^  [a-z_-]*:$' .github/workflows/ci.yml` after `jobs:`.

  GitHub Actions   — 10 jobs (workspace, lint, build, test, coverage, audit,
                     docs, integration, conformance, gate)
  Local simulation — 114 steps discovered across 5 crates (engine · mcp · cli ·
                     actions · server) from 180 scripts: 25 root + 87 engine +
                     28 cli + 40 actions
  Coverage         — workspace line coverage, gated ≥60%
```

The local suite is the same script set CI runs per crate, so a green local run and
a green pipeline measure the same thing.

```bash
# Run the full CI suite (114 steps, ~10 min)
bash .pi/scripts/local-ci.sh

# Run a specific stage
bash .pi/scripts/local-ci.sh --stage=lint      # lint only
bash .pi/scripts/local-ci.sh --stage=build     # build only
bash .pi/scripts/local-ci.sh --stage=test      # test only
bash .pi/scripts/local-ci.sh --stage=security  # security only
bash .pi/scripts/local-ci.sh --stage=docs      # documentation only
bash .pi/scripts/local-ci.sh --stage=integration  # integration only

# Run a specific crate
bash .pi/scripts/local-ci.sh --crate=engine
bash .pi/scripts/local-ci.sh --crate=mcp
bash .pi/scripts/local-ci.sh --crate=cli
bash .pi/scripts/local-ci.sh --crate=actions
bash .pi/scripts/local-ci.sh --crate=server

# Quick mode — skip release builds, use cargo check instead
bash .pi/scripts/local-ci.sh --quick

# Save report to a file (auto-gitignored under .pi/output/)
bash .pi/scripts/local-ci.sh --save

# List all available CI validation scripts
bash .pi/scripts/local-ci.sh --list
```

## Architecture Documentation

Each crate has its own `.pi/architecture/` directory with:
- **Module specs** — Detailed interface contracts for each bounded context
- **ADRs** — Architecture Decision Records explaining key design choices
- **Diagrams** — System context, data flow, deployment

### Published specifications

Versioned, citable specifications with enumerated non-claims and a falsifiable
validation protocol live in [`docs/spec/`](docs/spec/):

- [Consequence Gating v1.0](docs/spec/consequence-gating-v1.md) — dispatch-time
  preconditions: jurisdiction, evaluation contract, evidence model, interfaces,
  failure semantics, non-claims
- [Validation Protocol v1.0](docs/spec/validation-protocol-v1.md) — falsifiable
  cases with stated failure conditions
- [Terminology](docs/spec/terminology.md) — the normative vocabulary

> **Claim ceiling.** Public claims are bounded by these specifications. A passing
> test is *engineering verification* (L1) under a stated fixture; it does not
> silently inherit standing as *protocol validation* (L2) or *independent
> evidence* (L3). See [docs/spec/README.md](docs/spec/README.md) §Claim ceilings.

---

## Releases

Release history lives in [CHANGELOG.md](CHANGELOG.md) — one entry per released version,
newest first, each listing the pull requests merged and the issues closed in that
window. It is deliberately **not** reproduced here: this page is the product page,
and a version list belongs in the changelog.

The release train is `rigorix-engine` / `rigorix-mcp` / `rigorix-cli`, which version
together. `rigorix-actions` follows its own cadence and is not part of the train.

---

## Contributing

We welcome contributions! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for our development process, coding standards, and pull request workflow.

Key guidelines:
- Every edit must pass `cargo clippy --workspace` and `cargo fmt --check`
- All modules follow Clean Architecture with frozen contracts (see `.pi/architecture/`)
- Run `cargo test --workspace` before submitting
- New features require architecture documentation (see `.pi/prompts/feature-development.md`)

---

## License

Licensed under either of:

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
