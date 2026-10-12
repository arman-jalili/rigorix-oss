# Consequence Gating — Validation Protocol v1.0

| | |
|---|---|
| **Identifier** | `rigorix-consequence-gating-protocol` |
| **Version** | `1.0` |
| **Date** | 2026-10-12 |
| **Specifies** | [`rigorix-consequence-gating` v1.0](./consequence-gating-v1.md) |
| **Layer** | **L1 — Engineering verification** (see spec §6) |

A protocol is *falsifiable* only if each case states, in advance, what would
disprove it. Every case below is reproducible by an independent reviewer from a
public checkout, without running the engine's private state, and each has an
explicit **fail condition**. Cases 1–3 are mandatory for any version claiming
conformance; cases 4–7 cover the Consequence Gating–specific properties.

> **Claim ceiling.** Passing this protocol establishes bounded implementation
> behavior under the stated fixtures and version. It does not establish L2
> (protocol validation) or L3 (independent evidence), and a passing case does not
> silently inherit standing at those layers.

Fixtures are referenced by repository path. Where a fixture is described as
"in-memory", the reviewer constructs it from the description; no operator file is
required.

---

## Case 1 — Refusal on a negative check (the core property)

**Claim (spec §2.1, §2.4):** a matched step whose check does not pass is refused,
and the step's tool is never invoked.

- **Setup.** A plan with one step whose tool matches a precondition; a check that
  exits non-zero; a tool-invocation spy.
- **Procedure.** Dispatch the step.
- **Pass criterion.** The step's terminal state is a refusal; the spy recorded
  **zero** tool invocations; a `precondition_findings[]` entry exists with
  `outcome = failed`.
- **Fail condition (disproof).** The tool is invoked, or the step dispatches, or
  no finding is recorded. *Any of these disproves the claim.*

## Case 2 — Determinism

**Claim (spec §2.3):** identical step + config + check process yield an identical
verdict.

- **Setup.** A step, a precondition, and a check whose behaviour is a pure
  function of its stdin.
- **Procedure.** Evaluate the step N ≥ 3 times with the same inputs.
- **Pass criterion.** Every evaluation yields the same outcome and the same
  `inputs_hash`.
- **Fail condition.** Any two evaluations differ in verdict, or `inputs_hash`
  varies with identical inputs.

## Case 3 — Domain neutrality

**Claim (spec §2.6):** a new domain requires a new operator check, not an engine
change.

- **Setup.** Two preconditions over *different* domains (e.g. a payment
  eligibility check and a medical-consent check), each an operator argv program.
- **Procedure.** Evaluate each; confirm the engine's behavior is identical for
  both and that neither domain term appears in engine source.
- **Pass criterion.** Both are enforced by the same primitive; the only
  difference is the operator program; `grep` for the domain terms in the engine
  crate returns no gate logic.
- **Fail condition.** Enforcing the second domain requires a code change in the
  engine, or a domain term appears in the gate.

## Case 4 — Non-inheritance, both directions

**Claim (spec §2.7):** a provider's permitting answer is evidence, not authority.

- **4a — provider permits, our rule denies.**
  - **Setup.** An external determination service returning `permit`; an operator
    rule that denies (non-zero check, or a missing `require_params`).
  - **Procedure.** Dispatch the step.
  - **Pass criterion.** The step is **refused**. The provider's permit did not
    stand on its own.
  - **Fail condition.** The step dispatches because the provider permitted.
- **4b — provider denies, our rule permits.**
  - **Setup.** A provider returning `deny`; an operator rule that permits (exit
    0).
  - **Procedure.** Dispatch the step.
  - **Pass criterion.** The step is **refused**.
  - **Fail condition.** The step dispatches because our rule permitted.

> **This is the single most important case. If a provider's ALLOW ever stands
> alone, the feature has failed regardless of what else works.**

## Case 5 — Fail-closed on every ambiguity

**Claim (spec §2.5, §5):** unreachable, timeout, non-2xx, unparseable, and unknown
determination vocabulary all refuse, each distinguishable.

- **Setup.** A provider stub switchable across the five ambiguity modes.
- **Procedure.** Dispatch once per mode.
- **Pass criterion.** Every mode refuses; each refusal carries a distinguishable
  attribution reason; the tool is never invoked.
- **Fail condition.** Any mode dispatches, or two modes are indistinguishable in
  the record.

## Case 6 — Absent attribution is not tampered

**Claim (spec §3.1):** absent attribution fields mean a pre-attribution envelope.

- **Setup.** Two envelopes: one produced by a pre-attribution engine (no
  `check_digest`/`authority_digest`/`attribution`), one by the current engine.
- **Procedure.** Verify both with a published verifier.
- **Pass criterion.** Both verify; the pre-attribution envelope is **not**
  reported as tampered.
- **Fail condition.** The pre-attribution envelope fails verification, or is
  reported as tampered.

## Case 7 — No absolute paths in the signed record

**Claim (spec §3.2):** no message reaching the signed record carries an absolute
path.

- **Setup.** An operator installation under an arbitrary absolute path; a check
  that triggers a trust-boundary refusal, a boundary-strength refusal, a spawn
  failure, and a config-invalid error.
- **Procedure.** Produce each refusal; inspect the signed `summary`/`unarmed_detail`.
- **Pass criterion.** Every message names artifacts by basename + one-way path
  hash; no absolute path (no leading `/`, no home directory) appears in the
  signed bytes. The full path is present only in the local log.
- **Fail condition.** Any absolute path appears in the signed record — **including
  via a composition root's `unarmed_detail`**.

---

## How to reproduce (independent reviewer)

1. Check out the tagged release named in the spec header.
2. Run the repository tests that implement the cases above; each is named
   `protocol_case_*` or is linked from the spec's §8 table.
3. For cases requiring a provider, use the in-memory stub described in the case;
   no network is required.
4. Compare against the **fail condition** of each case. A case that cannot state
   its fail condition is not part of this protocol.

## What this protocol does NOT do

- It does not validate against a live environment (L1 evidence only).
- It does not certify compliance (out of scope, spec §1 NC-8).
- It does not prove the operator's authority artifact is true (NC-4).
- It does not close the check→action window (NC-5/NC-6).

## Revision

| Version | Date | Change |
|---------|------|--------|
| 1.0 | 2026-10-12 | Initial protocol, 7 cases. |
