# ADR-024 — The execution receipt and the signed attestation

**Status:** Proposed — **every `Decision:` line below is `undecided` until the operator writes it.** A section whose
  `Decision:` is `undecided` blocks the issue that needs it; the issue is not implementable in that section, and a
  worker must stop rather than choose (#1527, #1528, #1529).
- **Date:** 2026-11-13
- **Owner:** operator (all decisions). Implementation: `rigorix-engine` 1.15.0 (tracked as #1526–#1529).
- **Bounded context:** **Execution** — `orchestrator` (the entry point and its DTO) and `execution-engine` (dispatch),
  with `effect-commitment` (records and the seam), `state-persistence` (the durable state), `audit` (signature) and
  `gate` (verdict shape). The receipt lands as **one new module** inside that context — module layout is §10.
- **Supersedes:** nothing. Extends ADR-002 (TOML template format), ADR-009 (error handling), ADR-013 (deterministic
  gating of composed actions), ADR-016 (audit integrity anchor), ADR-019 (effect commitment), ADR-020 (approval store).
- **Derived from:** `rigorix-harness` `engine-gap-register.md` E4, E9, E11, E12, E13, E14; `build-plan.md` §2–§3.

---

## Context — the artifact, in one page

Today the engine accepts an intent and produces a plan; a host that wants to supply its own steps calls
`run_from_template` with a `Vec<TemplateStepDef>` (`orchestrator/application/dto/mod.rs:178-192`), and
`build_graph_from_steps` chains them (`orchestrator/application/orchestrator_impl/helpers.rs:217`, asserted by
`orchestrator_impl/tests/plan.rs:130`). The step type has no dependency field, no declared effect set, and no
expectation. The result after a run is a `ValidationIterationReport` carrying counts, failures, tokens and duration —
no effects, no expectations, no request digest (`plan_validation/domain/report.rs:158-176`). "The execution did what was
asked" is therefore a claim made by the side that asked.

This ADR fixes two engine-owned artifacts and the rules around them:

| | artifact | produced by | contains | signed |
|---|---|---|---|---|
| **R0** | `Receipt` | the host, outside the engine | steps + `depends_on`, declared effects, expectation per step, workspace identity, sandbox regime, role-model identities, budget, `schema_version` | no — digest only |
| **R1** | `Attestation` | the engine, and only the engine | `digest(R0)`, `execution_id`, per-step observed effects (paths + hunk digests), per-step expectation outcomes, `quality { level, source }`, settled effect ids, identity, signature | yes |

Verification is `digest(R0) == digest(R0 as echoed in R1)` **and** every expectation holds; the refusal is one decision
about the *composed action*, not a judgement about each step in turn — the shape ADR-013 established for sequence
policy. A refusal is wholesale: no partial verdict, nothing dispatched. E4 (the seal), E9 (signed evidence) and E12 (gate evidence) are
properties of these two types, not separate deliverables.

## Decision

This ADR is decided in five parts, each with its own `Decision:` line: **§2** D1 the gate level, **§3** D2 the key
model, **§4** D3 the receipt's field set, **§5** D4 the workspace identity, **§6** E4 the protected store, **§7** E13
where the settlement lives, **§8** the budget's meaning. Until a part is written, the issue that needs it is not
implementable in that part and a worker must stop rather than choose — the options and their consequences are below so
that writing them is an edit, not an investigation.

**Not in scope here:** executing the quality level (that is D1's answer, and either way it is a separate change),
criteria *semantics* (the criteria module, `build-plan.md` Phase 2), and anything a role reads or writes as content.

---

## 2. D1 — is the gate level executed or attested?

The gate level today is a **caller claim**: `evaluate_gate` evaluates `input.observed_level`
(`quality_gates/application/service_impl.rs:49-63`), and `classify_test_scope` takes four booleans —
`workspace_tests_run`, `lint_passed`, `format_passed`, `audit_passed` (`:63-93`). Nothing in `engine/src` spawns
`cargo test`; the 43 `process::Command` sites are git tools, `run_command`, hooks, the precondition runner and
`scored_evaluation`. Every `cargo` string is an allow-list entry or a fixture.

| option | what it means | test that proves it | consequence |
|---|---|---|---|
| **D1-a executed** | the level is steps the engine runs in the sandbox and cites by id in R1 | a receipt whose level-carrying steps ran produces `quality.source = Engine` with those step ids; a receipt that asserts a level with no steps produces `source: Caller` and no passed marker | the harness's fail-closed claim becomes honest; needs a `cargo test` step type in the sandbox regime (E3's mechanism), so it is Phase 2 work |
| **D1-b attested only** | the level is recorded as the caller's claim, `source: Caller`, never as passed | the same test, other direction: `source` is never `Engine` unless a step in *this* execution produced it | shippable in Phase 1; "merge-ready" then means "the caller declared it", and no document may say otherwise |

**Either way, the determinate rule holds and is not a decision:** no field may carry `source: Engine` unless a step in
that execution produced it. That is what makes D1-b honest and D1-a meaningful.

**Recommendation:** D1-b for Phase 1 (it is what #1527 already specifies and it is honest), with D1-a as the Phase 2
change that promotes `source` from `Caller` to `Engine` for the four gates.

**`Decision:`** undecided — operator.

---

## 3. D2 — the signing key model

| option | what a verifier can conclude | precedent in the tree |
|---|---|---|
| **D2-a local HMAC** | "this attestation was produced by a holder of the key, and has not changed since" — tamper-evidence, no third-party proof; the verifier must hold the same secret, so the harness and the engine share it | `approval/domain/record.rs:144-180` (node-bound, single-use, HMAC, TTL) and `audit/application/envelope_factory_impl.rs:168 sign()` |
| **D2-b asymmetric** | "this was signed by the engine's private key" — verifiable by anyone with the public key, which is the only form in which an attestation is evidence to a party that does not trust the harness | none in the tree; new dependency and key handling |

**Recommendation:** D2-a for 1.15.0 (the tree already has the idiom and no key-distribution story), stated in the ADR
and in every document as *tamper-evidence, not third-party proof*. D2-b is the change that makes a receipt portable to
a verifier who does not run the harness — which is the distributed-execution story, and its own ADR.

**`Decision:`** undecided — operator.

---

## 4. D3 — the receipt's field set and `schema_version` policy

One schema, one parser: today `[[nodes]]` (engine registry, `depends_on`) and `[[steps]]` (`tool` +
`[steps.parameters]`, every demo repository) describe one artifact and are bridged in the host crate
(`mcp/src/template_tools/infrastructure/template_converter_impl.rs:79,314`; #1529). R0 is an instance of that one
schema, so the field set below is a subset of it.

| field | type | produced by | refusal when |
|---|---|---|---|
| `schema_version` | `u16`, monotonically increasing | host | `SchemaVersion` — an unknown value is refused, never coerced |
| `execution_id` | `Uuid` | host | half of `#1528`'s resume refusal |
| `workspace` | **D4's type** — see §5 | host | `WorkspaceMismatch` |
| `sandbox` | `{ regime: container \| vm, image_digest }`; `none` is not representable | host | `SandboxMissing` (`shell_policy.rs:33-34,604-640`) |
| `steps[]` | `{ id, tool, parameters, depends_on[], declared_effects { paths[], content_digests }, expectation, requires_approval, timeout_secs }` | host | `UnknownStep`, `DependencyCycle`, `SecretInParameters` |
| `roles[]` | `{ role, model_id, context_window, structured_output }` | host | capability contract mismatch (harness-side) |
| `budget` | `{ max_iterations, max_tokens }` | host | see §8 — declared, and echoed by R1 |
| `quality_required` | the four-level vocabulary (`quality_gates/domain/level.rs:29-43`) | host | R1 records `{ level, source }` per §2 |
| `set_digest`, `fixture_digests[]` | `Sha256`, over the canonical form | host computes, engine pins (§6) | `DigestMismatch`, `ArtifactTampered` |

**Policy to decide with the field set:** is `schema_version` a single integer with "unknown = refuse" (strict, and the
only policy that cannot silently mis-parse), or `major.minor` with unknown-minor tolerated? And may a receipt ever be
re-sealed after it has been used (the current answer, which #1527 asserts, is no: a different digest for the same task
is refused).

**Recommendation:** one integer, unknown = `SchemaVersion` refusal; no re-seal, ever. Both are the strict reading of
"a version number must denote exactly one artifact".

**`Decision:`** undecided — operator. **Blocks:** #1529 (the parser needs the field set), #1527 (the type is the field
set).

---

## 5. D4 — what identifies a workspace

Qualification is defined on "the unmodified repository" (`build-plan.md` Phase 2), and R0 carries the workspace it
applies to. `RunInput.repo_root` already exists (`orchestrator/application/dto/mod.rs:35`), so the *mechanism* is a
parameter; what is missing is its **identity**.

| option | identifies | an uncommitted tree | cost |
|---|---|---|---|
| **D4-a commit tree OID** | `git rev-parse HEAD^{tree}` | cannot be named — a receipt over a dirty tree is unrepresentable | simplest; the harness must commit or refuse; a task that legitimately starts from a dirty tree is out of scope |
| **D4-b tree OID + dirty digest** | the tree OID plus a digest over `git status --porcelain=v1 -z` plus the content digests of the changed paths | nameable, and precise | the host must produce it; a harness bug in the dirty digest is a silent wrong-identity (test: two checkouts with one differing byte must differ) |
| **D4-c tree OID + declared exceptions** | the tree OID and an explicit list of paths that may differ | nameable for a named set only | a new attack surface: the exception list is content the side being judged can propose |

**Evidence:** the engine's file tools already carry a `workspace_root` with canonicalisation and containment
(`tools/application/file_read_tool.rs:80-113`) and git tools carry `repo_root`, so D4-b/c need no new capability —
only a function that computes the identity and is used in exactly one place.

**Recommendation:** **D4-a** in 1.15.0. It is the only identity that is a single, already-existing git answer, it makes
"the unmodified repository" mean something exact, and dirty-tree qualification is not needed to close Phase 1. If a
later phase needs it, D4-b is an additive change with its own refusal (`WorkspaceDirty`).

**`Decision:`** undecided — operator. **Blocks:** #1527's `workspace` comparison — its acceptance must not state
`HEAD^{tree}` (an earlier revision did; that was the issue deciding D4 for the operator).

---

## 6. E4 — the seal and where it lives (the protected store)

The requirement: the digest over the criteria and every fixture is recomputed before evaluation, a mismatch refuses the
whole evaluation, a different digest for the same task after sealing is refused, and **the sealed copy resolves to a
location the executing agent cannot write**. A seal inside the agent-writable tree is not a seal: the agent lowers the
bar before it is evaluated.

What exists, measured:

- **No protected-path concept.** `permission_mode` is `read_only | workspace_write | dangerous_full_access`
  (`configuration/domain/config.rs:97`); `excluded_dir | protected_path | deny_write | readonly_path` → **0 hits** in
  `engine/src`.
- **A durable store already exists, and both hosts put it inside the repository.** The state manager is a
  `FileSystemStateRepository` rooted at a `state_dir` (`state_persistence/application/state_manager_factory_impl.rs:31-46`),
  and both hosts root it at `<repo_root>/.rigorix/state`:
  `mcp/src/engine_wiring.rs:66-67` and `mcp/src/host/mod.rs:1667-1670`. Approval records are appended to that persisted
  state (`approval/infrastructure/mod.rs:8`), and execution records are JSON files with atomic tmp+rename writes
  (`state_persistence/infrastructure/filesystem_execution_record_repository.rs:21-64`).
- **The agent's write tools are contained to `workspace_root`** — which is exactly why "inside the repo" is reachable.

So the mechanism is not a new adapter. It is a **location rule plus a refusal at construction**:

1. `state_dir` must not be inside `repo_root`. Canonicalise both and refuse to build the orchestrator if
   `state_dir.starts_with(repo_root)` while sealing is armed — `CoreOrchestratorError` variant `StateDirInsideWorkspace`,
   naming both canonical paths.
2. The sealed receipt and its fixture digests are written **once** into the state dir, and every verification reads them
   from there, never from a path R0 supplies. R0 supplies the digest; the engine resolves the bytes.
3. Re-sealing the same `(execution_id | intent_key)` with a different digest is refused (`SealedDigestChanged`).

| option for the location | consequence |
|---|---|
| **E4-a a sibling directory** — `<repo_root>/../.rigorix-state/<repo-hash>` | nothing new to configure; survives `git clean`; a different user with the same path can read it, so it is tamper-*evidence* only |
| **E4-b an operator-declared path** (`RIGORIX_STATE_DIR`), required when sealing is armed | explicit, auditable, and the only option that can be placed outside the user's home on a shared host; breaks `init`-and-go for existing users unless defaulted |
| **E4-c a per-user store** (`~/.local/state/rigorix/<repo-hash>`) | outside every workspace by construction; the agent may still be running as the same user, so it is a boundary against in-repo tools, not against a hostile process |

**Recommendation:** E4-b with E4-a as the default. The rule is what matters (not inside the workspace, resolved by the
engine, written once); the default should keep `init` working.

**The circularity this closes:** if R0 names the path of the seal, the agent writes the path. If the engine resolves the
seal from its own store by digest, the agent can only break the digest — which is a refusal, not a pass.

**`Decision:`** undecided — operator (location rule + default). **Blocks:** #1527 P1.5 and the DoD bullet "a sealed
criteria set cannot be replaced by a writable copy".

---

## 7. E13 — where the settlement lives

`effect_commitment` settles **per record** and does so durably before the effect is sent
(`effect_commitment/application/seam.rs:40-55`), but the *execution* is left unstated. #1528 adds one
`ExecutionSettlement` per execution and refuses a resume while an `Ambiguous` settlement exists.

The store matters more than the type. The effect ledger's port has exactly two adapters — `InMemoryEffectLedger`
(`in_memory.rs:47-70`: a `HashMap` behind a `Mutex`) and `HttpEffectLedger` (needs a running service)
(`in_memory.rs`, `http.rs`; config `application/config.rs:15-16`: `kind = "in_memory" | "http"`). A settlement written
"beside the effect records" therefore **evaporates on a crash in the default local configuration**, which is the exact
event the DoD tests.

| option | consequence |
|---|---|
| **E13-a the state repository** — settlements as a new record type in the filesystem state repo (§6's store) | durable locally with no new adapter; the ambiguity refusal works on the harness's machine; consistent with approval records, which already live there |
| **E13-b the effect ledger's port** — a third adapter, or a file adapter behind the existing port | keeps the effect vocabulary in one place; a new adapter for one record type; still needs a location outside the workspace |
| **E13-c the HTTP ledger only** — settlements require the commitment service | the only way it is durable without new code; adds a service dependency to the harness, which the design had avoided |

**Recommendation:** **E13-a.** It is the smallest change that satisfies the DoD, it inherits §6's location rule instead
of inventing a second one, and it keeps the effect ledger's port about effects.

**`Decision:`** undecided — operator. **Blocks:** #1528's in_scope wording and its crash fixture.

---

## 8. The receipt's budget: declared, and echoed

R0 carries a budget (roles, iterations, tokens). The engine's `LlmBudget` is consumed on the **planning** path
(`planning/application/{mock_classifier,mock_extractor}.rs` take `&LlmBudget`; `error.rs:90` maps
`LlmBudgetError`) and dispatch reserves **capacity** through the capacity ledger
(`execution_engine/application/service_impl/dispatch.rs:1201-1243`) — neither is the receipt's budget. A declared limit
with no consumer is the defect class this ADR exists to make impossible: the harness would believe a run was bounded.

Two coherent ends, and one is not allowed:

- **Declared and observed (recommended for 1.15.0):** R1 reports `budget { declared, observed }` — iterations used and
  tokens used are already counted by the loop (`plan_validation/domain/loop_config.rs:31-48`) — and the attestation
  says `enforced: false`. No document may then claim a task ran within its budget.
- **Enforced:** dispatch refuses a step that would exceed the declared budget, with a typed refusal, before dispatch.
  That is a new enforcement point and its own issue.
- **Not allowed:** a `budget` field that nobody reads and no attestation echoes.

**`Decision:`** undecided — operator (declared-and-observed, or enforced now).

---

## 9. Invariants the implementation must satisfy (these are not decisions)

1. **Wholesale refusal.** A receipt that fails any check dispatches nothing and produces no attestation. No partial
   verdict, no "verified but skipped" state.
2. **One author per artifact.** `Attestation` has no public constructor, no `Default`, no `Deserialize`; only the
   engine's builder produces it. `Receipt` may be deserialised by anyone and verified by anyone.
3. **Observed, never asserted.** Every field of R1 is a value the engine computed or read from the effect ledger; none is
   copied from R0 except `digest(R0)` and the identity it was dispatched under.
4. **Typed refusals, redaction at construction** (ADR-009). No path, secret or parameter value in a refusal unless it is
   the subject of the refusal.
5. **Every refusal has a fixture that asserts the variant**, not a sentence.
6. **Digest canonicalisation is one function**, used by the receipt, the fixtures and the attestation; two
   canonicalisations is the same defect as two schemas.

---

## 10. Module layout — not a new bounded context, one new module

This is not a new bounded context. The receipt's language *is* the execution language — execution, dispatch, effect,
settlement, quality, identity — and the harness's ubiquitous language already places `Receipt`, `Receipt digest`,
`Attestation` and `Execution settlement` in the **Execution** context. What the engine's Execution context spans today is
already five modules: `orchestrator` (the entry point and the DTO), `execution-engine` (dispatch), `dag-engine` (the
graph), `effect-commitment` (the seam and records), `state-persistence` (the durable state), with `audit` (signature) and
`gate` (verdict shape) beside them. A new context would claim a new language; there is no new vocabulary here.

It is also not two modules. `Receipt` and `Attestation` are one chain: R1's first field is `digest(R0)`, `verify`
compares them, and §9.6 requires **one** canonicalisation function. Two directories would put that comparison across a
module boundary that has nothing else in it, and add a second module doc and a second `pub mod` for one artifact pair.

| option | layout | consequence |
|---|---|---|
| **§10-a one module** | `engine/src/receipt/` with `domain/{receipt,attestation,refusal,digest}.rs` and `application/{verify,sign}.rs`; `pub mod receipt` in `lib.rs`; `modules/receipt.md` whose header reads `Module: engine/src/receipt/` and `Bounded context: execution — what was asked, and what the engine observed` | one canonical form, one public verify surface, 40 `pub mod`, 39 module docs |
| **§10-b two modules** | `engine/src/receipt/` and `engine/src/attestation/` | the digest function and its tests live in one and are used by the other; two module docs; the chain's comparison crosses a boundary |
| **§10-c extend `execution-engine`** | no new directory; types in `execution_engine/domain/`, services in its application layer | nothing new to register; the module grows a data contract that callers (hosts, harness) must import from the dispatcher, and a public verification surface lands inside the largest module |

**Naming:** the module is `receipt` and the type is `Receipt`, while `effect-commitment` already has
`ProviderReceiptReader` — the effect provider's answer. That is a term conflict to record in the ubiquitous language:
*provider receipt* is always qualified, *receipt* is R0. The central concept keeps the plain word.

**Recommendation:** **§10-a.** It matches the precedent that `gate` and `effect-commitment` exist as small modules
precisely because they own one contract and one seam, it keeps the digest function single, and it gives the harness one
small, stable dependency to import.

**`Decision:`** undecided — operator. **Blocks:** #1527's `file_changes` (its paths follow this choice; the current list
is §10-a).

---

## 11. What this ADR does not decide

- The criteria module's semantics (`build-plan.md` Phase 2): `Qualify`/`Final`, two runs, per-criterion stability,
  pristine workspace, the four aggregates.
- Whether the quality level becomes executed (D1-a) — decided in §2 as *input*, implemented separately.
- Who may query the attestation and settlement after the fact, and for how long. The version in this ADR is: whoever can
  read the state directory, which E4's answer makes a filesystem permission question rather than a protocol one.
- The harness's own contexts, roles, retrieval and wiring (`engine-gap-register.md` §3).
