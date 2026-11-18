# ADR-025 — One governed assembly, and the tree that proves it is the only one

**Status:** Accepted — decided 2026-11-17; recorded here on 2026-11-18 when the six issues that cite it
  (#1625–#1630) were opened, because a decision six issues name by path and the tree cannot read is a decision
  that only exists in a conversation. Each decision below is one edit to override, and every one names the
  measurement that forced it.
- **Date:** 2026-11-17 (decided) / 2026-11-18 (recorded)
- **Owner:** operator (all six decisions). Implementation: `rigorix-engine`, tracked as #1625–#1630.
- **Bounded context:** **Execution** — `gate` (the composition root), `orchestrator` (the entry point),
  `execution-engine` (dispatch), `receipt` (the declaration and its observation), and the three hosts that
  assemble them (`mcp`, `cli`, `actions`).
- **Supersedes:** nothing. Completes the half-step ADR-017 took in #1381, and extends ADR-013 (sequence
  policy — one composed action with two owners is a defect), ADR-024 (the receipt is the authority the run
  must satisfy) and ADR-016 (audit integrity anchor — the host's regime is recorded, not assumed).
- **Derived from:** four assemblies measured in the tree (below), plus the two defaults measured in
  `execution_engine/application/factory.rs:200` and `orchestrator/application/orchestrator_impl.rs:355`.

---

## 1. Context — one machine, four assemblies

The engine has one orchestrator and four places that assemble it. Measured, not estimated:

| assembly | lines | what it attaches |
|---|---|---|
| `mcp::host::build_real_engine` (`mcp/src/host/mod.rs:1375`) | 2 297 | precondition gate, sequence policy, capacity, effect commitment, approval store, quality gates, scored evaluation, sandbox |
| `actions::main` (`actions/src/main.rs:396`, `:402`) | 1 284 | precondition gate, sequence policy, quality gates, scored evaluation, sandbox |
| `cli::cli_boundary::orchestrator` (`:346`) | 888 | `GateSetup::from_env` — the composition root the engine built for exactly this |
| `receipt::api::run_receipt` (`engine/src/receipt/api.rs`, #1619) | 417 | audit key, permission mode, state dir, sandbox. **No precondition gate, no sequence policy, no capacity, no effect commitment, no approval store, no quality gate, no scored evaluation** |

`GateSetup`'s header already recorded why the engine took the *order* on in #1381: "Moving the order into the
engine while leaving the construction in each caller would only move half the problem … the next runtime would
have to rediscover it." Four assemblies later the tree shows what "leave the rest to review" produces, and the
consequence is not stylistic: a host that finds `run_receipt` — the only one of the four that looks like an
API — obtains an engine that consults no precondition at dispatch, and cannot tell. **"No findings" and "no
gate" are the same output.**

Two defaults make the situation reachable from inside the engine as well: `ExecutionEngineConfig::precondition`
is `None`, and `OrchestratorServiceImpl::new` accepts a caller-built execution service. So "this engine consults
ADR-017's gate at dispatch" is a property of a source file, not of a type.

## 2. Decision

Six decisions, each with the constraint that forces it and the artifact that makes it checkable. Nothing here
waits on an operator choice; §8 lists what this ADR deliberately does not decide.

### 3. D1 — the crate is the boundary, and it publishes **one** assembly

A host outside this workspace depends on `rigorix-engine`, not on the MCP surface. Therefore the governed
assembly is an engine symbol, documented at the crate root with the same care as `receipt`, and the four
assemblies above are **retired into it**: after #1625, `mcp`, `actions`, `cli` and `receipt::run_receipt` each
*call* it, and exactly one production call site constructs an orchestrator with the governed seams.

**Decided by derivation.** Two facts measured in the tree. First, the receipt's own argument applies to
itself: "a builder invites a caller to assemble an engine missing its audit service and discover it at the
first refusal" — `run_receipt` assembles an engine missing its *consequence gate*, and that failure mode is
silent rather than late. Second, the surface a stranger reaches is the crate: the packaged-artifact check
already compiles a consumer against the tarball, and the MCP tool descriptor's input is a frozen five-name
contract that is not the engine's own (§8 D6).

**Shape.** A single `async fn`, not a builder — the same shape `run_receipt` chose, for the same reason. Its
arguments are the things a host may not delegate (§3 of #1619: `state_dir`, the signing key, the permission
mode) plus the workspace root and the host's name, and it *returns* the assembled services rather than
requiring the caller to reach back into a half-initialised object. What a host brings is data with no
`with_*` chain: a planning pipeline, and the seam-free extras that are the host's business (audit service,
quality-gate service, scored evaluation, policy engine, code graph).

**Not delegable, and therefore without a default:** the workspace root, the state directory, the attestation
signing key, the permission mode. Each is a fact that, defaulted wrongly, produces a run that looks governed
and is not — a state directory inside the workspace lets the agent rewrite the seal (ADR-024 §6), a missing
key is an unsigned observation, and a permission mode that was never chosen is an enforcer that was never
built (#1487).

### 4. D2 — prefer the type over the guard, and keep the guard for what the type cannot see

A type decides what can be written; a source guard decides what was written. The project uses both, and D2
fixes the division:

- **The type** is a witness, produced only by the assembly, that the orchestrator constructor *requires*. An
  orchestrator that has not passed through the governed seams then does not typecheck, and removing the
  witness from the assembly is a compile error in every host rather than a runtime absence.
- **The guard** is a source test with a named allow-list, because a type cannot reach `cfg`-gated code,
  macro-generated code, or a host that adds a dependency and writes its own import.

**Decided by derivation.** #1498 established "gate on the capability, not the name"; #1513 that "a registry,
not a pattern, decides"; #1522/#1530/#1531 that two gates answering one question must not disagree. A witness
without the guard is reusable by anyone who constructs the constructor's argument by other means; a guard
without the witness reports a file and line after the code exists. Neither alone answers "can a host reach an
ungoverned engine?", so both are required, and the guard's allow-list is a file in the tree with a comment per
entry — a regex in a test is a judgement with no record.

**The escape hatch is named.** Some callers legitimately need an ungated orchestrator (tests, the portable
slice, a deliberately ungoverned probe). The alternative constructor is `Ungoverned::new`, the name is the
documentation, and the guard counts its call sites so the list cannot grow quietly.

### 5. D3 — the receipt is the runbook

`run_receipt` takes `&Receipt` and **derives** the plan. It does not take a caller-supplied step list beside
the declaration.

**Decided by derivation.** `ReceiptStep` already carries `id`, `tool`, `parameters`, `depends_on`,
`declared_effects`, `expectation`, `requires_approval`, `timeout_secs` — every field a step needs, with two
renames (`id`→`name`, and `expectation.description` where the DTO wants a description). While `steps` is a
parameter, a host can run a plan its receipt does not describe, and the only thing that notices is a
comparison whose failure arrives *after the effects*. ADR-024 replaced "a checklist kept beside a plan
somebody else wrote" with "the declaration is the authority the run must satisfy"; a surface that takes both
is the old arrangement with a new type name.

**Total, not best-effort.** The mapping is exhaustive destructuring over `ReceiptStep`, so a future field is a
compile error at the mapping site rather than a silently dropped property, and a step the mapping cannot
produce (unknown tool, dependency on an absent id, approval required with no approval store) is a typed
refusal before any dispatch. Repair is not available: this is a declaration, and a declaration the engine
completes on the author's behalf is not one.

### 6. D4 — a rule with two owners has one owner: the store

The attestation store has `put` and no `get`, so `run_receipt` composes `state_dir/attestations/<id>.json`
itself. Two files then hold one location rule, and the copy outside the store is the one that gets edited
when the layout changes.

**Decided by derivation.** #1455 established the principle in the gate: "routing them through this struct
would give one decision two owners". #1569 established it for the settlement, which stores one rule in two
vocabularies. The measurement here is internal and asymmetric: `FileReceiptStore` already has `get`, so the
declaration is readable through its store and the observation is not.

**The rule.** A location convention is the store's; a caller holds no path. `get` returns the same
`StoredAttestation` the writer returned, so a reader and a writer describe the record in one vocabulary, and a
typed miss distinguishes "never sealed" from "cannot read" — a distinction the product needs, because a host
displaying a completed run and a verifier handed a digest from last week both ask it.

### 7. D5 — the seam order is enforced by the build, not by review

Every governed seam is attached in one function, in the engine, and the tree fails when that function is
bypassed: a host adding a hand-wired `PreconditionSetup::from_env` or a direct orchestrator construction turns
the suite red with file:line.

**Decided by derivation.** This is the historical record of the tree itself: #1381 moved the order into the
engine and left the construction to callers with a paragraph explaining the risk; four assemblies later, the
assembly written *after* `GateSetup` existed (`run_receipt`) consults no gate at all. A published assembly is a
convenience until something fails when it is bypassed. The instrument is deliberately the cheapest in this
ADR — a source test in the shape `tests/execution_single_path.rs` proved — because its cost is what decides
whether it survives.

**What a seam means here.** Attached in the assembly, in this order, for every host: precondition gate,
sequence policy, capacity ledger, effect commitment, approval binding, permission enforcer, sandbox
declaration, recovery recipes, then the dispatch and planning services that consume them, then the seal
(attestation store, sealed state directory) and the audit service. The order is not a preference: a gate
attached after the executor is built is not consulted, which is the #1381 finding repeated one layer down.

### 8. D6 — the MCP façade carries a receipt because the MCP is a host

`rigorix_run` accepts an optional receipt: validated against the crate's schema, refused with the crate's
typed refusal, and reported back as `receipt_digest`/`attestation_digest` when supplied.

**Decided by derivation.** #1619 left `receipt: None` and wrote the reason into the tree: "an R0 behind a tool
parameter is a contract change across five published channels". D6 keeps that reasoning and fixes the framing.
The reason the façade should carry a receipt is not that MCP is the engine's only edge — a host outside this
workspace depends on the crate (D1) and never sees this surface. It is that a tool which runs consequential
steps and reports a digest should be able to receive the declaration whose digest it reports; today `None` is
honest and indistinguishable from unarmed.

**Additive, and frozen where it is frozen.** The five `EXECUTION_TOOL_NAMES` do not change. The parameter is
optional; a call without it still reports `receipt_digest: None` and now *says* which of three states that is
(no receipt / not sealed / store refused). The refusal vocabulary a caller already branches on
(`precondition_checked`, `precondition_findings[]`, `precondition_denied`) does not change, and a receipt
refusal is its own code rather than a reuse of those.

## 9. Module layout and the artifacts that make this checkable

| decision | artifact | where |
|---|---|---|
| D1 | the assembly, native-only, documented at the crate root | `engine/src/gate/infrastructure/` (+ the packaged-surface check naming it) |
| D2 | witness type + `Ungoverned::new` + compile-fail doctest | `engine/src/gate/domain/`, `engine/src/orchestrator/application/` |
| D2/D5 | source guard + `--self-test`, allow-list as data | `engine/tests/`, `engine/.pi/architecture/assembly-allowlist.tsv` |
| D3 | the mapping, total over `ReceiptStep`; refusals | `engine/src/receipt/application/service.rs`, `engine/src/receipt/api.rs` |
| D4 | store read side; no path literal outside the store | `engine/src/receipt/infrastructure/attestation_store.rs` |
| D6 | optional parameter + three named "no digest" states | `mcp/src/execution_tools/`, `docs/spec/specs.json` |

`portable` keeps refusing all of it at build time, by name: the assembly, the witness and the receipt store
all sit outside the portable slice, which takes its ports from the host by construction. The packaged-surface
check asserts the refusal, so a consumer learns the boundary from a build rather than a run.

## 10. Consequences

- **A host learns governance from the type it must use, not from a document.** The failure mode this ADR
  removes is silent: an engine that consults no gate produces the same output as an engine whose gate found
  nothing.
- **Every host's diff gets smaller.** `mcp`'s 2 297-line composition loses its seam block; the CLI, which
  already used `GateSetup`, loses nothing and gains the rest of the assembly.
- **One thing deliberately gets harder:** constructing an orchestrator by hand. That is the cost, and it is
  the point — the constructor now demands evidence that some code owning the seam order said yes.
- **A refusal stays a refusal.** Nothing in this ADR repairs a declaration; every new refusal path is typed,
  and the vocabulary a caller already branches on does not move.

## 11. What this ADR does not decide

- **Which gate ports are mandatory.** ADR-017 and ADR-013 answer that; the assembly attaches what the
  operator's files arm and refuses what they malform, and does not invent a policy of its own.
- **Receipt schema 3.** D3 adds no field and removes none: `ReceiptStep` already says everything.
- **Planning from an intent.** The `run()` path is unchanged; D3 is about the receipt-driven path.
- **Streaming, listing, and directory walks.** D4 adds one read door, not a query language.
- **Where the harness keeps its own receipts.** That is the host's module, and the harness is the first host
  that needed `run_receipt` at all.