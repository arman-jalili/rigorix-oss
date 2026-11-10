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
14. **The Python verifier has no effect-commitment verification.** All three channels
    publish `rigorix-verifier` 0.9.0 (crates.io, npm, PyPI — checked against each
    registry, 2026-11-10), and the Python package verifies the chain, signatures and the
    three anchor artifacts. It does not verify an effect-commitment record: the Rust
    verifier has since 0.7.0, and the Python API has no equivalent. A PyPI consumer may
    verify evidence integrity and may not verify a commitment claim.
15. **Enterprise has no tagged release.** It is deployed continuously; there is no
    version number to cite for it.

## 4. Commercial non-claims

16. No hosted multi-tenant service.
17. No SOC 2 report, ISO 27001 certificate or equivalent attestation.
18. No customer references, production telemetry or case studies.
19. No claim of peer-reviewed validation of this specification or the
    implementation.
20. No claim beyond this document set. These requirements bind an implementation that
    claims conformance to this specification. They make no statement, favourable or
    otherwise, about any other system, framework or term.

## 4a. Boundary non-claims (added in 0.4.0)

21. **No containment from a declaration.** `RIGORIX_SANDBOX=container` is a statement the
    operator makes; the engine does not inspect the namespace it runs in. What a
    conforming implementation establishes is that an *undeclared* step does not run —
    never that a declared step is contained (RS-099, RS-100).
22. **No control-plane protection inside the workspace.** `.rigorix/**` is refused on the
    resolved path, and a symlinked alias of it is refused as the directory itself. That
    is the extent of it: a configuration file an agent can write, and whose write the
    engine does not see, is a control the agent holds. `.git/config` and `.git/hooks`
    are exactly that, which is why a version-control step requires a declared isolation
    (T13).
23. **No guarantee that a "read" step only reads.** The implementation refuses the
    subcommands and flags known to write (RS-107) and refuses a form list of
    program-naming options. It cannot promise that no future flag of a read subcommand
    writes; the list is a control, not a proof.
24. **No claim about the file, from the import table.** Six imports that no decision
    reads establish a capability bound for the recorded artifact. They establish nothing
    about whether *this* host answered them with constants — that is the embedder's
    claim (RS-103, RS-106).
25. **No reproduction across toolchains, claimed as one digest.** A digest identifies an
    artifact built on a stated platform with a stated toolchain. A Linux build and a
    macOS build of one commit are different artifacts, and a specification that published
    one number for both would pin neither.

## 5. How to use this document

- Any public claim is checked against §2–§4 before publication.
- Any claim that a competitor makes can be evaluated with the same list.
- If a claim requires a non-claim to be softened, the non-claim is right and the claim
  is wrong until the underlying work is done.

The list is expected to shrink. Items are removed only when the corresponding
evidence exists and is linked from `11-traceability.md`.
