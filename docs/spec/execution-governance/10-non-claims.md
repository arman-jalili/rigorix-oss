# 10 — Non-claims

**Status: normative.** A statement that contradicts this document is void, regardless
of where it appears.

---

## 1. Why a non-claims document exists

An architecture describes what is designed. A product claim describes what is true in
a deployment. The gap between them is where trust is lost.

This document lists what is explicitly not claimed, so that any document, page,
talk or sales conversation can be checked against it.

## 2. Technical non-claims

1. **No elimination of the check-to-effect window.** A precondition narrows the gap
   between decision and consequence. It does not close it. Downstream systems must
   enforce at their own commit point.
2. **No domain truth.** The engine does not know whether a beneficiary is eligible, a
   payment lawful, a change safe, or a dataset sensitive. Authority comes from
   operator systems.
3. **No sandbox.** A dispatched command runs as a subprocess. Process isolation,
   containment and host hardening are deployment concerns.
4. **No transactionality.** If a downstream system partially applies an action, the
   engine does not roll it back.
5. **No protection against a compromised host.** An attacker with the operator's own
   credentials and filesystem access is outside the model.
6. **No correctness of operator checks.** A wrong check produces a wrong decision,
   correctly recorded.
7. **No immutability.** Evidence is append-only and tamper-evident. A privileged
   actor can delete or rewrite storage; what changes is detectability, not
   impossibility.
8. **No self-proving evidence.** A self-held key produces integrity, not independence.
   Only a separate anchor holder produces external provenance.

## 3. Capability non-claims (current reference implementation)

9. **Effect scope is evidenced, not necessarily refused.** The oracle is wired in
   CLI, MCP and Actions and the plan declares effect scope, but the current
   behaviour reports an out-of-scope effect; it must not be described as a hard
   block until a refusing path is verified.
10. **Cross-session capacity is not available in the stateless engine.** The
    in-process ledger provides single-execution atomicity. Distributed mode requires
    the control plane and a contested store.
11. **Posture reporting covers the seams the doctor knows.** A new seam that is not
    added to `rigorix doctor` is invisible to the same discipline that closed RS-052.
12. **Commitment has two independent ceilings and both must be stated.** An
    in-process record proves the unsettled interval within one process only. Settled
    is provable only with a provider receipt (L3). Without one, the honest claim is
    *not-settled*.
13. **Kill/restart semantics under L1.** A process restart loses the in-process
    record; orphaned `Dispatched` records are swept, but the sweep is a recovery
    mechanism, not a settlement proof.
14. **The Python verifier is behind.** `rigorix-verifier` is 0.8.0 on crates.io and
    npm, and 0.6.0 on PyPI. Until the PyPI job runs (blocked by CI quota until
    November), the Python package cannot verify effect-commitment evidence.
15. **Enterprise has no tagged release.** It is deployed continuously; there is no
    version number to cite for it.

## 4. Commercial non-claims

16. No hosted multi-tenant service.
17. No SOC 2 report, ISO 27001 certificate or equivalent attestation.
18. No customer references, production telemetry or case studies.
19. No claim of peer-reviewed validation of this specification or the
    implementation.
20. No claim to define, supersede, or speak for any other author's framework.
    Execution governance is a shared category name; this document specifies one set of
    requirements under it and defines those terms as used here. A reader comparing
    frameworks should use the published sources listed in `README.md`, not this
    document's summary of them.

## 5. How to use this document

- Any public claim is checked against §2–§4 before publication.
- Any claim that a competitor makes can be evaluated with the same list.
- If a claim requires a non-claim to be softened, the non-claim is right and the claim
  is wrong until the underlying work is done.

The list is expected to shrink. Items are removed only when the corresponding
evidence exists and is linked from `11-traceability.md`.
