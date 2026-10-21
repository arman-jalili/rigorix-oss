# Rigorix

[![rigorix-verifier on crates.io](https://img.shields.io/crates/v/rigorix-verifier?label=rigorix-verifier%20%C2%B7%20crates.io)](https://crates.io/crates/rigorix-verifier)
[![rigorix-verifier on npm](https://img.shields.io/npm/v/rigorix-verifier?label=rigorix-verifier%20%C2%B7%20npm)](https://www.npmjs.com/package/rigorix-verifier)
[![rigorix-verifier on PyPI](https://img.shields.io/pypi/v/rigorix-verifier?label=rigorix-verifier%20%C2%B7%20PyPI)](https://pypi.org/project/rigorix-verifier/)
[![CI](https://github.com/arman-jalili/rigorix-oss/actions/workflows/ci.yml/badge.svg)](https://github.com/arman-jalili/rigorix-oss/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green)](LICENSE-MIT)

**AI agents are moving out of chat windows and into production systems — running
migrations, modifying code, executing payments. Prompt engineering and network
proxies cannot stop a rogue action, and they cannot see a *sequence* of
individually-permitted actions that together do something nobody authorised.
Rigorix is the deterministic execution boundary underneath your agents: it
compiles intent into a reviewable plan, separates the moment of authorisation from
the moment of consequence, refuses at dispatch when the operator's own check says
no, and records every decision in a cryptographically anchored audit trail.**

That is the whole product in three sentences. If your agents can move money, ship
code, or touch regulated data, Rigorix is the layer that decides what they are
**allowed** to do and proves what they **did** — with no LLM in the enforcement
path.

It is for the security, platform and risk teams who are being asked to say "yes"
to autonomy and are being asked to *prove* it afterwards. It is not an agent you
chat with, and it is not a faster autocomplete: `Claude Code`, `Cursor`,
`Copilot`, `Aider` and `Codex` keep their own loop and plug into Rigorix over MCP
as **the layer they run inside**.

---

## Why the controls you already have do not cover this

| You have | It governs | The gap |
|---|---|---|
| **Prompt engineering** ("never touch prod") | What you *ask* the model | A suggestion the model is free to ignore. Guidance is not a boundary. |
| **An API gateway / network proxy** | Bytes flowing to and from the model | Not what the agent does in your repository, shell, CI or payment path. |
| **Conversation history** | A transcript | Not evidence: not signed, not chained, not tamper-evident, not attributable. |
| **Static policy on the request** | One request in isolation | Not the *composition*: five permitted steps whose sequence is a prohibited outcome. |
| **A logging / SIEM pipeline** | Telling you afterwards | It reports. It cannot refuse. |

The failure mode is not a malicious model. It is a **well-behaved agent doing
individually-permitted things in an order nobody authorised** — and a control
plane that cannot tell the difference between "an action was allowed" and "this
sequence must not happen".

Rigorix exists to make that distinction enforceable and provable.

---

## The architecture in one picture

```
  intent (natural language)
        │
        ▼
  ┌───────────────────────────────────────────────────────────────┐
  │ PLAN-TIME       compile to a reviewable DAG; apply operator    │
  │                 requirements and sequence rules BEFORE any      │
  │                 side effect (ADR-013, ADR-015)                  │
  ├───────────────────────────────────────────────────────────────┤
  │ APPROVAL-TIME   a human authorises an INTENT, not a node;       │
  │                 HMAC-bound, single-use, time-bound (ADR-011)    │
  ├───────────────────────────────────────────────────────────────┤
  │ DISPATCH-TIME   ONE choke point. Re-verify the intent, run the  │
  │                 operator's precondition against live authority, │
  │                 then dispatch — or refuse (ADR-011, ADR-017)    │
  └───────────────────────────────────────────────────────────────┘
        │                                    │
        ▼                                    ▼
   signed envelope                    authority-fact projection
   (HMAC + chain + anchor,            (verified by RE-DERIVATION,
    ADR-016)                           spec §4.3.1)
```

The LLM decides *what to generate*. The plan — not the model — decides *what is
allowed to happen*. Enforcement is deterministic Rust.

---

## The eight decisions that define the product

Each of these is a shipped decision record. They are listed in the order a
reviewer should read them, because each one closes a hole the previous one leaves
open.

### 1. Risk gating, budgets and autonomy presets (ADR-004, ADR-007, ADR-008)

Every execution runs inside operator-authored risk policy, permission and budget
boundaries. Budgets are **counted per execution and evaluated before each tool
call**: a call that would exceed its limit is refused with `budget_exceeded` and
the tool never runs. Limits come from `[enforcement] preset` in `rigorix.toml`
(`default` = 500 tool calls, 3600 s, 100k tokens), and per-tool `max_calls` is
enforced from the same counters.

Contested capacity is different and stronger: it is **reserved before dispatch and
released after**, so a crashed or cancelled run cannot leak its allowance
(ADR-018). See the demo below, where the refused step's tool never executes.

Autonomy is a *preset*, not a vibe: the operator picks the envelope, the engine
enforces it.

### 2. Approval that binds to an intent, not to a screen (ADR-011)

A human approval is not a checkbox on a node. At approval time the engine captures
the canonical `ExecutionIntent` from the sealed graph and stores
`HMAC-SHA256(run_key, canonical(tool ‖ intent ‖ declared_scope))` as a
**single-use, time-bound** record. Immediately before dispatch it re-derives the
intent and compares. Match → dispatch and consume. Mismatch → **HALT**
(`IntentMismatch`): no dispatch, an audit event, re-approval required.

This closes three real replay vectors: cross-process resume against a tampered
state file, render/approve/execute mismatch, and stale approval sets reused after
a re-plan.

*Honest boundary:* there are two binding classes, and they are not equally strong.
Deterministic tools (`run_command`, `file_write`, `edit_file`, `git_*`) are
**payload-anchored** — the full dispatch payload is bound, airtight at dispatch
level. `llm_generate` is **input-anchored** — the prompt and context are bound; the
generated output is non-deterministic and verified *post hoc* by the validation
loop. Binding governs agent-mediated tool dispatch; it is not a claim about what an
arbitrary subprocess does afterwards.

### 3. Composed-action gating — the sequence, not just the step (ADR-013, ADR-014, ADR-015)

This is the answer to sequence attacks. Declarative rules over **ordered step
sequences**, evaluated deterministically:

```toml
# .rigorix/sequence-policy.toml  — operator-owned; agents may not write it
[[sequence_rules]]
steps = ["read_customer_record", "payment_execute"]
action = "promote"          # promote to human approval (default) | deny
```

The default action is `promote`, not `deny`: the product posture is *Rigorix does
not ask — it refuses, or gates the step for human approval*. `deny` is available
where the operator wants a hard block.

Two evaluation points, deliberately: **plan-time** (primary — the ordered step
list is evaluated *before* the graph is sealed, so the composition is caught
before any side effect) and a **run-time prefix gate** in the same dispatch choke
point, for dynamically composed plans.

ADR-015 adds **operator-controlled step requirements** — evaluated for *every*
plan, independent of what the plan itself declares. ADR-017 R3 closes its
non-goal: a consequential step can require a **companion step** to be present in
the same plan, so "payout only if the authority recheck also ran" is expressible
and enforced.

ADR-014 adds **value-identity**: a rule can compare a value to one carried by an
earlier matched step in the same run (`/beneficiary` equals the earlier step's
`/beneficiary`), plus a canonical **effect key** so the same economic action is
recognised across runs and identifiers.

*Honest boundary:* identity resolution and analytics live **outside** the engine.
The domain supplies a canonical effect key; Rigorix does not guess what
"beneficiary" means. There is no LLM in this path.

### 4. Consequence gating — a precondition at the moment of dispatch (ADR-017)

The strongest primitive: an operator authors a **dispatch-time precondition** — a
deterministic external check, argv (no shell) or an HTTP determination service —
evaluated immediately before the matched step's tool is invoked, at the same
single choke point as the approval re-verification.

```toml
# .rigorix/preconditions.toml — operator-owned
[[preconditions]]
id = "beneficiary-eligible"
match = { tool = "payment_execute" }
require_params = ["/beneficiary", "/amount"]      # absent → deny
command = ["/opt/rigorix/checks/beneficiary-eligible"]   # argv, resolved OUTSIDE the agent-writable workspace
action = "deny"                                    # refuse; or "promote" to gate
```

The answer is **never cached across the gap**: authority at T₀ is not authority at
Tₙ, so the check is re-read per dispatch.

**Non-inheritance is normative (R4).** A provider's permitting answer is
*evidence, not authority*: there is deliberately **no "provider says allow"
short-circuit**. Every other matching precondition is still evaluated, an unknown
or newer determination vocabulary maps to `unknown`, and **unknown never becomes
authority**. Unreachable, timeout, non-2xx, unparseable and unknown all refuse.

**Step-outcome gating (R2)** makes the consequence of failure explicit:
`release_dependents_on_failure = false` means a failed (or denied) step marks its
transitive dependents `Skipped`; they are never dispatched.

*Honest boundary, stated plainly:* a precondition **narrows** the window between
check and consequence; it does not close it. For money-out, the downstream system
must still enforce eligibility at its own commit point. What Rigorix guarantees is
narrower and reliable: **when the operator's check says no, the step is refused,
and the determination is signed.**

### 5. Evidence that survives the agent: HMAC, chain, anchor (ADR-016)

"The audit trail" is not one artifact. Rigorix splits it by **trust role**:

| Tier | Home | Properties | Trusted for decisions? |
|---|---|---|---|
| **Ledger** | external anchor (enterprise DB / external sink) | append-only, agent-inaccessible, per-producer sequence and chain | **yes** |
| **Projection** | a signed read from the ledger | narrow, verifiable ("prior actions in window W for scope S") | **yes**, after verification |
| **Cache** | local filesystem / `rigorix-server` | fast, rebuildable, chain-linked | tamper-**evident**, **not** trusted |

Two principles do the work:

- **Evidence authority ≥ rule authority.** A rule sourced from the enterprise
  cannot be enforced against local, deletable evidence.
- **No symmetric secret across a trust boundary.** HMAC secures the cache *within*
  one trust domain; across host ↔ anchor, anchors are asymmetric.

*Honest boundary:* the local cache is **tamper-evident, not tamper-proof**. It is
not trusted for decisions; that is why the ledger tier exists.

### 6. Verification by re-derivation, not by trusting a signature (spec §4.3.1)

Rigorix can export its own hash-chained history as **authority facts** for an
external determination consumer — and the export is checked by **re-deriving it**
from the anchored chain, not by trusting a signature.

A signature proves only *who said it*. Re-derivation proves *what the history
actually supports* — so it also catches a signer asserting facts the chain does
not back. Absence is explicit: a subject with no record yields a
`NOT_ESTABLISHED` fact, and the artifact **never asserts present standing** (NC-4).

This is the differentiator a neighbouring product cannot match by publishing more
marketing: **the verifier is open source and published on crates.io, npm and
PyPI** (with Go and Java implementations), so the reviewer runs the check on their
own machine, in their own language, with no round-trip to the vendor.

```bash
rigorix audit projection --subject authority:beneficiary-eligible -o bundle.json
rigorix audit verify-projection bundle.json   # offline; no engine, no key
```

### 7. Identity: OSS attests, the enterprise authorises (ADR-012)

Rigorix records **who was presented as acting** — an attributed, time-bound claim,
best-effort verified when an IdP is reachable, and **explicitly marked
`unverified` or `absent` when it is not**. OSS makes no authorization judgment;
scope and role decisions belong to the enterprise layer, which consumes the same
signed envelope.

*Honest boundary:* this deliberately **fails open** on identity for a local
developer tool (with an explicit marker in the envelope), and it says so. That is
not the same decision as egress control, and the two are documented separately so
they are never conflated. Marking is not prevention: a verifier can *see* a forged
authority digest; prevention is the boundary itself.

### 8. Quality gates with pluggable scoring (ADR-010)

A `scored_evaluation` node evaluates an artifact against a rubric — inline or
referenced — with per-dimension thresholds and its own retry/fallback policy, over
a **Rigorix-defined scoring protocol**. External scoring systems adopt the
protocol by implementing the server side. Scoring participates in merge gating
like any other policy.

---

## What Rigorix is not

Stated because a security reader should be able to rely on a claim without
discovering a hidden gap.

- **Not a workflow engine.** The precondition primitive is exactly *match a step,
  run a check, refuse on failure, record the outcome*. No loops, no conditionals,
  no policy DSL, no multi-step orchestration.
- **Not a domain system.** The engine does not know what "beneficiary eligible"
  means. The operator supplies the process; entity resolution lives in the domain.
- **Not judgment by model.** No LLM in the enforcement path — by construction, in
  the risk gate, the sequence rules, the preconditions and the approval reverify.
- **Not a replacement for approval.** A human authorises the intent; the
  precondition re-validates the world. They are different controls.
- **Not a transparency log or gossip stack.** The signed envelope plus the anchor
  already provide the evidence.
- **Not a benchmark-backed claim.** No independent comparative evaluation
  exists. See [Where Rigorix fits](#where-rigorix-fits) for the failure modes
  this design distinguishes, and what that table does not establish.

---

## Where Rigorix fits

The useful comparison is not agent-vs-agent — it is **which credential-binding
failure mode a control actually closes**. Rigorix is a governance layer beneath
whatever agent you run, so it is measured on these dimensions:

| Failure mode | What breaks | Rigorix binding | Source |
|---|---|---|---|
| **Scope** | A credential is used outside the step it was approved for | Approval binds to `(tool, intent, declared_scope)`; dispatch-time preconditions re-read the authoritative state | ADR-011, ADR-017 |
| **Argument** | The dispatched arguments differ from the approved ones | Approval binds the exact intent bytes and re-derives them at dispatch | ADR-011 |
| **Temporal** | The decision was true at plan time but is stale at dispatch | Approval and preconditions are revalidated at the moment of consequence | ADR-017 |
| **Tool** | A permitted tool is swapped for a different one | Tool identity is part of the approval binding and of the effect key | ADR-011, ADR-014 |
| **Delegation** | Authority is silently re-delegated | The record is bound to the attesting identity; whether a delegation is legally valid is explicitly out of scope | ADR-012, spec NC-2 |
| **Semantic** | The effect differs from the intent | Canonical `eff:` effect keys and value-identity matching | ADR-014 |

**What this table is not.** It is not a benchmark, not a competitor ranking, and
not a claim of independent validation. No public independent comparative
evaluation exists; the [claim ceiling](#claim-ceiling) applies (L1 engineering
verification, L2 protocol validation, L3 independent evidence). Named-competitor
cells are deliberately omitted: a cell that cannot be sourced and dated would be
"not established", which is noise rather than evidence. Any vendor can be
measured against the six dimensions above, using its own documentation.

---

## Interfaces

**MCP gateway** (`rigorix-mcp`) — plugs Rigorix into any MCP-compatible assistant.
15 built-in tools: execution (6), templates (4), audit (4, including the
authority-fact bundle), usage guide (1); plus 3 auth tools when an identity
provider is configured.

**MCP is the adapter surface, deliberately.** There are no framework-specific
adapters (LangChain, OpenAI tools, CrewAI, …) and none are planned: a shim per
framework would be a second dispatch path with the engine's controls on the far
side of it, which is the failure this project exists to remove. Any MCP host —
stdio or Streamable HTTP — reaches every control; a host outside MCP integrates
through the server or the library, both of which are the same code.

```bash
cargo install rigorix-mcp
```

```json
{ "mcpServers": { "rigorix-mcp": { "command": "rigorix-mcp", "args": [] } } }
```

**CLI** (`rigorix-cli`) — plan, run, inspect, and verify offline: `run`, `plan`,
`cancel`, `status`, `history`, `explain`, `diff-plan`, `generate`, `template`,
`audit {list|show|diff|projection|verify-projection}`, `logs`, `config`, `doctor`,
`init`, `key`, plus the interactive TUI as the default entry point.

**Server** (`rigorix-server`) — the OSS reference host: JSON-RPC 2.0 over the
frozen `rigorix.*` catalog, with an optional MCP Streamable HTTP transport.

**GitHub Action** (`rigorix-actions`) — the same engine, adapted to CI, on its own
release cadence.

See [`mcp/README.md`](mcp/README.md), [`cli/README.md`](cli/README.md) and
[`engine/README.md`](engine/README.md).

---

## Quickstart

```bash
cargo install rigorix-cli
export ANTHROPIC_API_KEY="sk-ant-..."
rigorix init                       # scaffold .rigorix/ with a default rigorix.toml
rigorix plan "add retry to the payment webhook"   # plan first — review before running
rigorix run  "add retry to the payment webhook"
```

Operator policy lives in `.rigorix/` — `policy.toml`, `permissions.toml`,
`sequence-policy.toml`, `preconditions.toml`. **These are the trust surface, and
the default permission config denies executing agents write access to
`.rigorix/**`** — otherwise an agent could rewrite the rules it is judged by.

---

## See it run

One command, no Docker, no network. The first run compiles the engine (a few
minutes); after that it takes about a second:

```bash
git clone https://github.com/arman-jalili/rigorix-oss && cd rigorix-oss
demo/capacity-ledger/run.sh
```

It writes a throwaway `.rigorix/capacity.toml` and drives the real engine types —
no mocks, no recorded fixtures. Act 1 is the reserve/commit/release contract with
the aggregate exposure printed after every step; act 2 arms the same declaration
into the real executor and launches two executions that contend for one cap.

```text
── Act 1 — the contract ──

1. execution A reserves 3
   ▒▒▒··   committed 0, outstanding 3, available 2 of 5

…                       (retry with the same key: still one hold)

3. execution B reserves 3
   refused: insufficient capacity for resource 'payments-budget': requested 3, available 2
   ▒▒▒··   committed 0, outstanding 3, available 2 of 5
   unchanged — a refusal reserves nothing

── Act 2 — the seam, through the real executor ──
  hard_limit 4, step amount 3, two executions launched together
  dispatched: 1, refused before dispatch: 1
  tool markers on disk: 1 of 2 (the refused step never ran its tool)
  ███·   committed 3, outstanding 0, available 1 of 4
  refusal: capacity_denied (the step's tool was never called)
```

The marker file in act 2 is the point: the refused execution's tool **never ran**,
so its marker never appears. A local counter — run the tool, check afterwards —
cannot produce an absence. Scope of the guarantee, stated plainly: the in-process
adapter is **single-process** atomicity, not cross-session protection; that needs
the HTTP adapter against a real contested store (ADR-018).

Full transcript, and what the demo does *not* prove:
[`demo/capacity-ledger/`](demo/capacity-ledger/README.md).

---

## Claim ceiling

Public claims here are bounded by the published specifications. A passing test is
**engineering verification (L1)** under a stated fixture; it does not silently
inherit standing as **protocol validation (L2)** or **independent evidence (L3)**.

- [Consequence Gating v1.0](docs/spec/consequence-gating-v1.md) — the normative
  spec, including what each mode does *not* cover
- [Validation Protocol v1](docs/spec/validation-protocol-v1.md) — falsifiable
  claims with stated failure conditions
- [Terminology](docs/spec/terminology.md) — the normative vocabulary

The decision records live in
[`engine/.pi/architecture/decisions/`](engine/.pi/architecture/decisions/) — start
with ADR-011 (approval binding) and ADR-017 (consequence gating).

---

## Releases

Release history is in [CHANGELOG.md](CHANGELOG.md). The release train is
`rigorix-engine` / `rigorix-mcp` / `rigorix-cli`, which version together;
`rigorix-actions` follows its own cadence.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Every edit must pass
`cargo clippy --workspace`, `cargo fmt --check` and `cargo test --workspace`;
documentation must pass `.pi/scripts/validate-docs.sh`.

---

## License

Licensed under either of MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](LICENSE-APACHE)), at your option.
