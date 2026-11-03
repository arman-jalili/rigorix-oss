# Ubiquitous Language

> Canonical glossary for **rigorix-oss**.
> All code MUST use these terms. Aliases/synonyms listed below are **prohibited** in source identifiers.
> Drift is detected by `.pi/scripts/validate-ubiquitous-language.sh`.

## Glossary

| Term | Definition | Bounded Context | Aliases/Synonyms | Examples |
|------|-----------|----------------|-----------------|---------|
| ReplaySession | The replayable unit: one scenario reference, one fixture world, one clock-reading series and one random seed, enclosing at most one attempt. | Portable Session | Attempt Session, session, run, playground session | `ReplaySession::open(scenario)` |
| GateAttempt | One proposed run submitted to the gate, identified by GateAttemptId; its outcome is not a business approval. | Portable Session | Attempt, request, run request | `session.attempt(action)` |
| RunOutcome | The result of one attempt: status plus the ordered StepOutcome list and the evidence references. No third shape and no host-settable verdict. | Portable Session | Outcome, result, verdict | `outcome.status == Refused` |
| StepOutcome | The decision for one gated step: allowed, refused or not reached, with the engine's gate id, reason and evidence reference. | Portable Session | step result, step status | `outcome.steps[1].decision` |
| ScenarioRef | A digest-bearing pointer to an engine scenario in the engine's own template and sequence-policy formats, never a re-typed policy. | Portable Session | policy, ruleset, scenario | `ScenarioRef::digest()` |
| FixtureWorld | The host-supplied immutable transcript of answers a port may give; data the policy evaluates, never authority. | Portable Session | mock data, test data, stub, fixture blob | `FixtureWorld::digest()` |
| PortFailure | A port's inability to answer (missing, exhausted, malformed, failed) which forces a refusal. For the identity port the engine's truthful status is `unverified`; a port failure is never reported as `unauthenticated`. | Portable Session | error, timeout, failure | `PortFailure::Identity` |
| ClockReading | A pinned instant answered by the clock port at a declared index; the only source of time in a session. | Portable Session | now, timestamp, wall clock | `readings[3]` |
| RandomSeed | The declared seed and recorded draw sequence that make randomness replayable. | Portable Session | nonce source, entropy | `seed.draw(2)` |
| EvidenceRef | A stable reference to the facts a record carries (what was checked, when, outcome, digests), borrowed from the engine and never minted here. | Portable Session | evidence, proof, receipt | `step.evidence_ref` |
| ArtifactSet | The exported, version-stamped byte sequence: envelopes, evidence references, scenario digest, engine version and artifact-format version. | Portable Session | export, bundle, record set | `export() -> ArtifactSet` |
| VerificationResult | Three-valued verdict on an artifact set: verified, not_verified, or unverifiable with a reason. | Portable Session | verified, valid, ok, invalid | `VerificationResult::Unverifiable(_)` |

## Adding New Terms

1. Identify the term used in conversation and code
2. Add a row to the Glossary table
3. Define the term's **bounded context** (which module it lives in)
4. List any **aliases/synonyms** that agents might mistakenly use
5. Provide **code examples** showing correct usage
6. Run `.pi/scripts/validate-ubiquitous-language.sh` to detect drift

> **Rule of thumb:** If two agents use different names for the same concept, add an entry.
> The canonical term is the one used in the architecture module documents.

## Term Conflicts (language gaps)

When the same term is used with different meanings by different parts of the
business (e.g. "transaction" = a database transaction for engineers but a
statement line for the business), that is a bounded-context boundary signal —
record the conflict here and resolve it in the exploration review:

| Term A | Term B | Why conflicting | Resolution |
|--------|--------|-----------------|------------|
| Attempt | GateAttempt | The engine already uses `attempt` for an approval-consumption marker and `retry_attempts`; the seam's unit is a whole run, not a marker. | adopt GateAttempt in this crate; `attempt` keeps its engine meaning and is not used for the seam's unit |
| Attempt Session | ReplaySession | `session` already means an auth session and an MCP session; a third meaning invites the wrong reading in logs and error text. | adopt ReplaySession |
| Outcome | RunOutcome | The engine already has `VerificationOutcome` and `PermissionOutcome`; a bare `Outcome` would be ambiguous in shared code paths. | adopt RunOutcome and StepOutcome |
| Evidence | EvidenceRef | The engine owns evidence (approval evidence, precondition findings); this crate only references it. | adopt EvidenceRef; never mint or rename evidence here |
| unverified | unverifiable | The engine's `unverified` describes an identity claim that could not be checked; `unverifiable` here is the three-valued verification verdict on an artifact set. They are different questions and must not be collapsed. | keep both, each in its own context and never rendered as the other |
