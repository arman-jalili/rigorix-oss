---
session_id: 3baf6e5c-e352-4127-af09-a5e7fe0a2d29
created: 2026-11-03
business_context: "This crate compiles a slice of the Rigorix engine to WebAssembly so that a host application (a browser demo today, a partner product later) can run the real gate locally: evaluate whether an AI agent's proposed action may proceed, refuse it with evidence when it may not, produce signed envelopes, and verify them offline. The engine already owns the domains; this crate owns the seam, the adapters and the guarantee that its output is byte-identical to the CLI's for the same input. WHAT THIS CRATE OWNS - a portable session: load a scenario from the engine's existing template and sequence-policy formats, plus a host-supplied business world (fixtures). - the adapter ports and their no-I/O implementations: clock, key source, provider transport (fixture-driven, no network), business sink (what a step would have written), randomness. - the host contract: attempt an action, receive an outcome (allowed, or refused with a business-worded reason and an evidence reference), export the artifact set, verify it. - the conformance guarantee against the shipped CLI. WHAT IT DOES NOT OWN (borrowed from rigorix-engine, never re-modeled) - precondition evaluation, sequence/requirement policy evaluation, approval binding, envelope construction and signing, identity attestation, the verifier. - the business domain: no cost sheet, no quote, no ledger semantics live here. ACTORS AND SYSTEMS - the host app: owns the UI and the mock business system; supplies fixtures. - the engine: owns the decision logic and the artifact format. - the approver (human) and the auditor/recipient who verifies a record without trusting the producer. - the CLI: the reference runtime. Same scenario, same artifacts, byte for byte. INVARIANTS THE EXPLORATION MUST ENCODE - Determinism: same scenario + same fixture answers + same clock input produce the same envelopes. Time and randomness are ports, never ambient. - The host cannot influence a decision. Anything a host supplies is data the policy evaluates, never authority. - Fail closed: a port that fails or is missing yields a refusal, never an allow. - No secrets, no absolute host paths, no personal data in artifacts. - An artifact produced in the browser verifies under the independent SDK (rigorix-verifier), which shares no code with this crate. QUESTIONS TO ANSWER - Which bounded contexts exist here, if any, and which concepts are value objects (attempt identity, scenario reference, evidence reference, outcome)? - What is the aggregate, if there is one, and what are its invariants? - Which domain events are produced, and are they evidence or telemetry? - Where exactly is the trust boundary between host, crate and engine, and what crosses it in each direction? - How is a port failure represented so the refusal vocabulary matches the engine's (an unverified session must not read as unauthenticated)? - What is the module decomposition, with the reason each module exists and the invariant it protects? - How is the CLI/WASM conformance property tested, including a seeded divergence that must fail the build? NON-GOALS - running shell commands, network calls, or filesystem access in the browser; - reimplementing any engine logic in JavaScript or TypeScript; - providing a server, an API, or persistence; - being the source of truth for gate behaviour (the engine is). ENVIRONMENT - target wasm32-unknown-unknown, no filesystem, no network, single-threaded; - size budget for the bundle, enforced in CI; - engine changes may only be additive behind non-default features, so the four published crates (engine, mcp, cli, actions) behave exactly as released; - the crate ships in the release train and publishes an npm package at the same version as the engine. , check other crates, architecture docs located in -pi to have a full picture"
status: draft
---

# Domain Exploration: 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

> **Status:** draft — AI-suggested, human-review needed.

---

## Business Context

This crate compiles a slice of the Rigorix engine to WebAssembly so that a host application (a browser demo today, a partner product later) can run the real gate locally: evaluate whether an AI agent's proposed action may proceed, refuse it with evidence when it may not, produce signed envelopes, and verify them offline. The engine already owns the domains; this crate owns the seam, the adapters and the guarantee that its output is byte-identical to the CLI's for the same input. WHAT THIS CRATE OWNS - a portable session: load a scenario from the engine's existing template and sequence-policy formats, plus a host-supplied business world (fixtures). - the adapter ports and their no-I/O implementations: clock, key source, provider transport (fixture-driven, no network), business sink (what a step would have written), randomness. - the host contract: attempt an action, receive an outcome (allowed, or refused with a business-worded reason and an evidence reference), export the artifact set, verify it. - the conformance guarantee against the shipped CLI. WHAT IT DOES NOT OWN (borrowed from rigorix-engine, never re-modeled) - precondition evaluation, sequence/requirement policy evaluation, approval binding, envelope construction and signing, identity attestation, the verifier. - the business domain: no cost sheet, no quote, no ledger semantics live here. ACTORS AND SYSTEMS - the host app: owns the UI and the mock business system; supplies fixtures. - the engine: owns the decision logic and the artifact format. - the approver (human) and the auditor/recipient who verifies a record without trusting the producer. - the CLI: the reference runtime. Same scenario, same artifacts, byte for byte. INVARIANTS THE EXPLORATION MUST ENCODE - Determinism: same scenario + same fixture answers + same clock input produce the same envelopes. Time and randomness are ports, never ambient. - The host cannot influence a decision. Anything a host supplies is data the policy evaluates, never authority. - Fail closed: a port that fails or is missing yields a refusal, never an allow. - No secrets, no absolute host paths, no personal data in artifacts. - An artifact produced in the browser verifies under the independent SDK (rigorix-verifier), which shares no code with this crate. QUESTIONS TO ANSWER - Which bounded contexts exist here, if any, and which concepts are value objects (attempt identity, scenario reference, evidence reference, outcome)? - What is the aggregate, if there is one, and what are its invariants? - Which domain events are produced, and are they evidence or telemetry? - Where exactly is the trust boundary between host, crate and engine, and what crosses it in each direction? - How is a port failure represented so the refusal vocabulary matches the engine's (an unverified session must not read as unauthenticated)? - What is the module decomposition, with the reason each module exists and the invariant it protects? - How is the CLI/WASM conformance property tested, including a seeded divergence that must fail the build? NON-GOALS - running shell commands, network calls, or filesystem access in the browser; - reimplementing any engine logic in JavaScript or TypeScript; - providing a server, an API, or persistence; - being the source of truth for gate behaviour (the engine is). ENVIRONMENT - target wasm32-unknown-unknown, no filesystem, no network, single-threaded; - size budget for the bundle, enforced in CI; - engine changes may only be additive behind non-default features, so the four published crates (engine, mcp, cli, actions) behave exactly as released; - the crate ships in the release train and publishes an npm package at the same version as the engine. , check other crates, architecture docs located in -pi to have a full picture

---

## Actors & Roles

| Actor | Description | Interactions |
|-------|-------------|-------------|
| Host application | Browser demo today, partner product later; owns UI, mock business system and fixture supply. | Calls the host contract: load scenario, submit attempt, export artifact set, request verification. Supplies only data (fixtures, clock input, randomness seed), never authority. |
| Rigorix engine (rigorix-engine) | Owner of the gate: precondition and sequence/requirement policy evaluation, approval binding, envelope construction/signing, identity attestation. | Consumed through additive, non-default features; never re-modeled. Returns findings, verdicts, envelopes, evidence references. |
| CLI reference runtime | The shipped rigorix CLI; the conformance oracle. | Produces golden artifact vectors for the same scenario, fixtures and clock; divergence fails the build. |
| Approver (human) | The person whose decision the gate consults; their decision is an input fact, not a host capability. | Approval is expressed as fixture/approval-binding data evaluated by the engine; the crate never grants approval. |
| Auditor / recipient | Verifies an exported record offline without trusting producer or host. | Uses the independent rigorix-verifier SDK on the exported artifact set; result is record-verified or not, never 'trusted'. |

---

## Functional Requirements

| ID | Requirement | Priority | Bounded Context |
|----|-------------|----------|----------------|
| FR-001 | The system shall open a Portable Session from an engine scenario reference (existing template and sequence-policy formats) plus a host-supplied fixture world, rejecting any input that does not parse under the engine's own types with a typed refusal rather than a default. | critical | Portable Session |
| FR-002 | The system shall accept a run request and return exactly one RunOutcome: a status (allowed | refused) plus the ordered StepOutcome list and evidence references. No step may be reported refused at the run level without the step that caused it, and no host-settable verdict shall be expressible. | critical | Portable Session |
| FR-003 | The system shall satisfy every engine-declared port (clock, key source, provider transport, business sink, randomness) exclusively from the session's fixture transcript, so that no attempt performs a network, filesystem, OS-time or ambient-entropy access. | critical | Portable Session |
| FR-004 | The system shall map any missing, exhausted, malformed or failed port answer to a typed PortFailure that becomes a refusal expressed in the engine's vocabulary — `unverified` for an identity port that could not be checked, never `unauthenticated` — and shall never map a port failure to an allow. | critical | Portable Session |
| FR-005 | The system shall export the produced artifact set (envelopes, evidence references, scenario digest) as a self-contained byte sequence containing no secrets, no absolute host paths and no personal data, and shall make that export the only supported input to verification. | high | Portable Session |
| FR-006 | The system shall export an artifact set that the host verifies with the independent rigorix-verifier SDK; the crate shall contain no verification logic and shall not depend on a verifier, so the producer cannot ship the checker (ADR-013). The three-valued vocabulary (verified / not_verified / unverifiable) is the host's to render, and the host contract states it. | critical | Portable Session |
| FR-007 | The system shall produce artifacts byte-identical to a CLI built from the same commit for the same scenario, fixture transcript and pinned clock/seed inputs, and shall fail the build when either the live CLI diff or a committed golden vector diverges. A skipped or missing vector is a failure, never a pass. | critical | Portable Session |
| FR-008 | The system shall run a seeded divergence vector (a deliberately altered fixture answer or serialization field) as a conformance test that is required to fail; if it passes, the build shall fail. | high | Portable Session |
| FR-009 | The system shall treat every host-supplied value (fixtures, approvals, provider answers, business-sink writes) as data evaluated by engine policy, and shall expose no API, flag or fixture key that can force an allow or suppress a finding. | critical | Portable Session |
| FR-010 | The system shall stamp each exported artifact set with the scenario digest, engine version and artifact-format version, and shall refuse to verify an artifact set whose format/version it does not support rather than guessing. | high | Portable Session |
| FR-011 | Deterministic inputs shall be supplied by the engine, not invented by this crate: an additive Clock and IdSource seam covering the gate path, defaulting to system time and reachable from the CLI behind a non-default feature. A feasibility spike precedes the crate: the engine's gate slice must compile to wasm32-unknown-unknown and one envelope's bytes must match the CLI's before Phase 0 begins. FR-007 is blocked until the seam lands; a normalised comparison is a label on a failing claim, never a pass. | critical | Portable Session |
| FR-012 | Conformance shall never compare approximately. A field that is nondeterministic must be pinned by FR-011, not normalised away before comparison; a normalised comparison is permitted only as a labelled interim and must be deleted, not relaxed, when FR-011 lands. | high | Portable Session |
| FR-013 | An approval shall enter as a claim the host supplies and the engine evaluates. The crate shall never grant an approval, and shall refuse a claim whose binding does not match the run's intent hash. | high | Portable Session |
| FR-014 | Every exported artifact set shall carry the artifact-format version and the engine version, and verification of an unknown format version shall return unverifiable with that reason. | medium | Portable Session |

---

## Non-Functional Requirements

| ID | Requirement | Category | Target |
|----|-------------|----------|--------|
| NFR-001 | The system shall be deterministic: repeated runs with identical scenario, fixture transcript and clock input produce identical artifact bytes within a process and across processes. | maintainability | 100% byte equality on the golden vector suite; zero ambient time/entropy/network/fs calls on the attempt path (architecture test fails otherwise) |
| NFR-002 | The system shall keep the shipped WebAssembly bundle within the release train's size budget, enforced in CI as a hard gate. | performance | Budget is set from a measurement, not a guess: the first green build's bundle size + 25%, enforced as a hard CI gate (regressions beyond the budget fail); no tokio runtime, reqwest, tree-sitter, prometheus, tracing-appender or libc in the wasm dependency graph |
| NFR-003 | The system shall keep secrets, key material, absolute host paths and personal data out of artifacts and out of any error or log message surfaced to the host. | security | Zero findings from a secret/PII/path scanner over exported artifacts and refusal messages in CI; key material exists only as an opaque test-key reference |
| NFR-004 | The system shall fail closed in three tiers: a logical failure (exhausted/missing/malformed fixture answer, unparseable scenario, unsupported format, unusable port) yields a typed refusal; a panic on the gate path is a defect that must not exist and is caught at build time by the panic-site baseline and deny-lints; a trap is surfaced to the host as fatal and is never rendered as a refusal or an allow (ADR-012). | availability | 100% of tier-1 injected faults return a typed refusal with zero allows; zero unclassed panic sites on the gate path; a host-contract test asserts a trap is fatal, distinct from refused |
| NFR-005 | The system shall require engine changes to be additive and behind non-default features, leaving the four published crates (engine, mcp, cli, actions) behaviourally unchanged, and shall publish at the engine's version. | maintainability | Default-feature test suites of all four crates green and unchanged per release; wasm npm package version strictly equal to rigorix-engine version |

---

## Assumptions

| Assumption | Impact if Wrong | Mitigation |
|------------|----------------|-----------|
| The engine can be compiled to wasm32-unknown-unknown behind additive, non-default features, with no filesystem, network or thread dependency on the exercised paths. | The crate cannot reuse engine logic and would be forced to duplicate it, breaking the source-of-truth invariant. | Feature-gated seam in the engine; a CI job builds the wasm target with default features of the four published crates unchanged; refuse to ship if any engine change touches a default path. |
| Every source of nondeterminism (time, identifiers, keys, randomness, provider answers, business world) can be enumerated as an explicit port and captured in a typed fixture transcript. Measured today: 292 Utc::now() call sites, 37 Uuid::new_v4() in the gate modules, and no Clock trait in the engine. | Browser and CLI envelopes diverge and determinism cannot be asserted or replayed. | Ports are the only ambient-capability access points, enforced by lint/architecture test; fixture transcripts are versioned and replayed in conformance vectors. |
| Byte-identical output across native and wasm is achievable with canonical serialization (stable map ordering, fixed field order, fixed hashing/encoding). | The conformance guarantee degrades to semantically-equal, and the whole value proposition of 'same artifacts' fails. | Canonical serialization owned by the engine; golden byte vectors checked per release; indexmap/ordered structure use audited. |
| The verifier (rigorix-verifier) publishes at the same version and can verify browser-produced artifacts with no shared code. | The 'verifies offline under an independent SDK' invariant is untestable. | Release train couples versions; CI runs the verifier against freshly exported browser artifacts on every release candidate. |
| A host cannot influence a decision through any input it supplies — fixtures are evaluated, not trusted. | The gate becomes host-controlled, invalidating the audit record. | Single attempt path with no host-settable verdict; any port failure maps to a refusal; a seeded 'host says allow' divergence test must fail the build. |
| The CLI can be given the same pinned clock and id sequence as the seam. | Byte-identical conformance is impossible, the central claim of the crate degrades to 'semantically equal', and the demo would be showing two different artifacts. | FR-011 (Clock + IdSource seam reachable from the CLI). Measured false today — there is no such input on the CLI — so FR-007 stays blocked until it lands, and the comparison is never quietly normalised to hide it. |

---

## Bounded Contexts

| Context | Description | Entities |
|---------|-------------|----------|
| Portable Session | The determinism seam: one run lifecycle bound to an engine scenario reference, a host-supplied fixture world and explicit port readings, replayable to identical artifact bytes. This is the only domain the crate owns. Modules it decomposes into: domain / application / interfaces (the published host contract and the wasm-bindgen surface) / infrastructure (the five deterministic adapters and the verifier bridge). Conformance is a build-time harness, not a context. | ReplaySession,GateAttempt,ScenarioRef,FixtureWorld,ClockReading,RandomSeed,PortFailure,RunOutcome,StepOutcome,EvidenceRef,ArtifactSet,VerificationResult |

---

## Entities

| Entity | Context | Type | Description |
|--------|---------|------|-------------|
| ReplaySession | Portable Session | aggregate-root | Aggregate root (identity ReplaySessionId): owns the scenario reference, the fixture-world pin, the pinned clock-reading series, the random seed, at most one GateAttempt, and the sealed ArtifactSet. |
| GateAttempt | Portable Session | entity | One proposed run under evaluation (identity GateAttemptId): carries the action and parameters, the resolved port answers and the RunOutcome. Never reopened or rescored, and it is not a business transaction. |
| ScenarioRef | Portable Session | value-object | Digest-bearing reference to an engine scenario in the engine's own template and sequence-policy formats; equal by digest and declared format version. |
| FixtureWorld | Portable Session | value-object | Immutable, versioned host-supplied transcript (provider answers, business-sink expectations, approval claims) that the engine evaluates as data; equal by transcript digest. |
| PortFailure | Portable Session | value-object | Typed record that a port could not answer (missing, exhausted, malformed, failed), carrying the port name and an engine-aligned failure kind; projects to a refusal reason. |
| ClockReading | Portable Session | value-object | One pinned instant plus its declared ordering index; the session's only source of time. |
| RandomSeed | Portable Session | value-object | The declared seed and the recorded draw sequence; equal by seed and draw digest, replayable to the same values. |
| RunOutcome | Portable Session | value-object | The single result of an attempt: status (allowed | refused) plus the ordered StepOutcome list and the evidence references. There is no host-settable verdict and no run-level refusal without the step that caused it. |
| StepOutcome | Portable Session | value-object | Per gated step: step name, decision (allowed | refused | not_reached), the engine's refusal kind and gate id, a business-worded reason, and the EvidenceRef it produced. |
| EvidenceRef | Portable Session | value-object | Stable reference to the facts a record carries — what was checked, when, with what outcome, which digests bind it. Borrowed from the engine, never minted here. |
| ArtifactSet | Portable Session | value-object | The exported byte sequence: produced envelopes, evidence references, scenario digest, engine version and artifact-format version. |
| VerificationResult | Portable Session | value-object | Three-valued: verified | not_verified | unverifiable(reason). Reason is populated only for unverifiable (unsupported format version, absent key, key not held). |

---

## Domain Events

| Event | Context | Description | Triggered By |
|-------|---------|-------------|-------------|
| ReplaySessionOpened | Portable Session | A session was accepted with a scenario digest, fixture-transcript digest, pinned clock series and random seed — the replay key. | OpenReplaySession |
| RunAllowed | Portable Session | The engine's gate permitted every step of the run; carries the evidence references and the approved action digests. | AttemptRun |
| RunRefused | Portable Session | The run did not proceed; carries the step that caused it, a business-worded reason and an EvidenceRef. | AttemptRun |
| StepDecisionRecorded | Portable Session | One gated step's decision was recorded inside the run — allowed, refused, or not reached. | AttemptRun |
| PortAnswerUnavailable | Portable Session | A port could not answer for this attempt; recorded as evidence of why the gate refused and as the fixture author's signal. | AttemptRun |
| RandomDrawConsumed | Portable Session | A randomness answer was drawn from the seeded sequence at a declared index. | AttemptRun |
| ArtifactSetExported | Portable Session | The sealed artifact set was exported, stamped with scenario digest, engine version and format version. | ExportArtifactSet |

---

## Ubiquitous Language

| Term | Definition | Bounded Context | Aliases/Synonyms |
|------|-----------|----------------|-----------------|
| ReplaySession | The replayable unit: one scenario reference, one fixture world, one clock-reading series and one random seed, enclosing at most one attempt. | Portable Session | Attempt Session, session, run, playground session |
| GateAttempt | One proposed run submitted to the gate, identified by GateAttemptId; its outcome is not a business approval. | Portable Session | Attempt, request, run request |
| RunOutcome | The result of one attempt: status plus the ordered StepOutcome list and the evidence references. No third shape, no host-settable verdict. | Portable Session | Outcome, result, verdict |
| StepOutcome | The decision for one gated step: allowed, refused or not reached, with the engine's gate id and reason. | Portable Session | step result, step status |
| ScenarioRef | A digest-bearing pointer to an engine scenario in the engine's own template and sequence-policy formats, never a re-typed policy. | Portable Session | policy, ruleset, scenario |
| FixtureWorld | The host-supplied immutable transcript of answers a port may give; data the policy evaluates, never authority. | Portable Session | mock data, test data, stub, fixture blob |
| PortFailure | A port's inability to answer — missing, exhausted, malformed or failed — which forces a refusal. For the identity port the engine's truthful status is `unverified`; a failure is never reported as `unauthenticated`. | Portable Session | error, timeout, failure |
| ClockReading | A pinned instant answered by the clock port at a declared index; the only source of time in a session. | Portable Session | now, timestamp, wall clock |
| RandomSeed | The declared seed and recorded draw sequence that make randomness replayable. | Portable Session | nonce source, entropy |
| EvidenceRef | A stable reference to the facts a record carries (what was checked, when, outcome, digests) — borrowed from the engine, never minted here. | Portable Session | evidence, proof, receipt |
| ArtifactSet | The exported, version-stamped byte sequence: envelopes, evidence references, scenario digest, engine and format versions. | Portable Session | export, bundle, record set |
| VerificationResult | Three-valued verdict on an artifact set: verified, not_verified, or unverifiable with a reason. | Portable Session | verified, valid, ok, invalid |

---

## Use Cases (Application Services)

| Use Case | Context | Description | Commands / Collaborators |
|-----------|---------|-------------|---------------------------|
| OpenReplaySession | Portable Session | Assemble a replayable session from an engine scenario reference and a host fixture transcript: parse with engine types, pin the clock series and seed, and reject anything incomplete with a typed refusal. | load scenario reference,validate fixture transcript,pin clock readings and random seed,bind attempt identity |
| AttemptRun | Portable Session | Drive one attempt through the engine's gate: resolve every port answer from the transcript, hand the run to engine policy evaluation, and project the RunOutcome with its ordered StepOutcomes and evidence references. A missing or failed port answer becomes a refusal, never an allow. | resolve port answers,invoke engine evaluation,project run outcome,record port failure as refusal |
| ExportArtifactSet | Portable Session | Serialize the sealed envelopes and evidence references into a self-contained artifact set, stamped with scenario digest, engine version and format version, scanned for secrets, absolute paths and personal data before it leaves — and handed to the host, which verifies it (ADR-013). | collect envelopes and evidence refs,stamp scenario/engine/format versions,scan and reject forbidden content,emit artifact bytes |
| RunConformanceVector | Portable Session | Replay a versioned scenario/fixture/clock vector through the wasm seam and through a CLI built from the same commit, compare artifacts byte for byte, and fail the build on any divergence or on a seeded divergence that does not diverge. | load golden vector,build CLI from same commit,replay both,compare bytes,assert seeded divergence fails |

---

## Open Questions

**None block module planning.** The five questions from the first draft, decided in review:

1. **Is the engine's wasm-facing slice reachable today?** No. Measured: no `cfg(target_arch = "wasm32")`, no wasm-bindgen, and the dependency graph contains blockers (`tree-sitter*`, `reqwest`, `prometheus`, `tracing-appender`, `libc`, and `ring` on the identity path). The seam defines an additive, non-default `portable` feature that swaps the adapters and drops the blockers for that target. Guard: the four published crates build and test unchanged from the same commit, and `cargo tree -e normal,no-dev --target wasm32-unknown-unknown` is diffed before and after.
2. **Does the seam surface an "unverified" state the host could misread?** Yes, and it must: three values (`verified`, `not_verified`, `unverifiable(reason)`), never a boolean. This is the #472/#1285 rule — `false` accuses, `null` reports a question that could not be asked — and FR-006 plus the invariant list make collapsing them a defect.
3. **Live CLI or committed goldens, and who re-baselines?** Both, doing different jobs: CI builds the CLI from the same commit and diffs live (authoritative, catches drift the day it lands); committed golden vectors serve the wasm-side test and the demo build, and are regenerated by that same job so a stale golden fails the live diff rather than waiting for a human. Re-baselining is a generated commit, reviewed like any artifact.
4. **What is the bundle size budget, and what is excluded?** Measured first, then `+25%` as the hard gate (NFR-002). Excluded by necessity: `tree-sitter*`, `reqwest`, `prometheus`, `tracing-appender`, `libc`; `ring` only if it turns out to sit exclusively on the identity/TLS path, verified with `cargo tree --target wasm32-unknown-unknown`. `wasm-opt` in the release build; `wasm-bindgen-cli` pinned to the crate version.
5. **Is approval binding expressible as fixture data?** The decision is data; the act is not this crate's job. The host supplies an approval *claim*; the engine binds it to the intent hash and refuses a mismatch (FR-013). The crate never grants an approval and models no human UI.

Tracked items, each with an exit condition (they do not block module planning):

- **The clock/id seam's call-site inventory** — FR-011 must name every site in the gate path that reads time or mints an id, and prove the published crates are unchanged. Exit: the inventory is a fixed list and the four crates' default-feature suites are green from the same commit.
- **`ring`'s reachability** — whether it is only on the identity/TLS path. Exit: `cargo tree` for the wasm target shows no `ring`, or the identity port is fixture-only and the dependency is provably unreachable.
- **Key ownership in the browser demo** — a visitor-generated key means the signature proves integrity and binding, not third-party non-repudiation. Exit: the demo's limits panel states it in one sentence, and the artifact stamp records which key produced the record.

**Decided in review (2026-11-03), and now binding:**

- **Who verifies — the host.** ADR-013 removes `VerifyExportedArtifacts` from the crate and `VerifierBridge` from infrastructure. The crate produces; the host checks with the independent SDK, in a process that never loads the wasm module. The independence claim becomes provable: the bundle's dependency tree contains no verifier.
- **The session-store port does not exist.** ADR-002 point 3 is withdrawn: verification's input is the `ArtifactSet`, not a session. A store port would exist only to persist a live aggregate and would invite a host to treat a stored session as evidence.
- **Failure semantics are tiered, not monolithic.** ADR-012 replaces NFR-004's single promise: refusals are typed and tested; panics are build-time defects (baseline + deny-lints); traps are fatal and host-visible. `catch_unwind` is documented as unavailable, not merely unused.
- **The crate is `rigorix-gate-wasm`, the npm package `rigorix-gate`, version-locked to the train, licence inherited.** ADR-011 adds the six registration points, including the `release.yml` tag loop that `rigorix-actions` fell out of for seven weeks.

---

## Aggregate Roots

ReplaySession