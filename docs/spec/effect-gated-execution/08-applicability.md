# 08 — Applicability across domains

**Status: informative.** The model is domain-neutral. This document shows the same
four axes applied to different classes of consequential action, so the model is not
mistaken for a payments product or a coding-agent product.

For each domain: what the consequential action is, how it fails, which operator
checks express authority, which axis matters most, and what the evidence must show.

---

## 1. Money movement and treasury

**Action:** a payment, transfer, refund, payout, settlement instruction.
**Failure:** the approval is valid, the beneficiary is frozen, the limit was lowered,
the account was closed. Or: the same beneficiary is paid twice under two identifiers.
**Operator checks:** beneficiary eligibility against the bank's own record; sanctions
and freeze lists; per-counterparty limits; duplicate detection by effect key.
**Dominant axes:** authority (T1), effect (T3/T12), capacity (aggregate exposure).
**Evidence must show:** the check that ran, the authority it consulted, the amount,
the beneficiary key, and the determination, signed at the moment of dispatch.
**Boundary:** settlement is downstream. The engine narrows the window; it does not
make the payment atomic.

## 2. Access, identity and entitlements

**Action:** grant a role, issue a credential, add a member, change a permission.
**Failure:** the approver approved access for one scope; the grant expands to a
broader one. Or: a joiner/mover/leaver sequence grants access after termination.
**Operator checks:** employment state at dispatch; entitlement policy per role;
separation-of-duties conflicts; scope comparison against the approved intent.
**Dominant axes:** authority (employment state), sequence (joiner → grant after
leave), effect (same access via a different group).
**Evidence must show:** the subject, the scope granted, the authority consulted, and
the approver identity.
**Boundary:** the IdP owns the truth. The engine ensures the grant does not outlive it.

## 3. Code, schema and infrastructure changes

**Action:** merge, deploy, migrate a schema, change a configuration, rotate a secret.
**Failure:** an agent performs a legitimate task and expands scope; a migration runs
outside the change window; a deploy proceeds after a freeze.
**Operator checks:** change-freeze calendar; required companion step (backup,
verification); protected path and scope checks; environment and blast-radius policy.
**Dominant axes:** sequence (backup before migration), authority (freeze window),
effect (same change via a different tool).
**Evidence must show:** the plan, the approved intent, the check outcomes, and the
diff or artifact identity where available.
**Boundary:** effect-scope evidence (which files changed) is opt-in in the reference
implementation and must be described as such.

## 4. Data operations and analytics

**Action:** export a dataset, delete records, change retention, run a destructive
query, train on sensitive data.
**Failure:** a permitted export composes with a permitted join into a re-identification
risk; a deletion proceeds after a legal hold; a retention change removes evidence.
**Operator checks:** legal hold at dispatch; data-classification policy; row/column
policy; query shape limits; destination allowlist.
**Dominant axes:** authority (hold state), composition (export + join), capacity
(row/volume budget).
**Evidence must show:** the dataset identity, the scope, the policy consulted, the
volume, and the destination.
**Boundary:** classification is domain truth. The engine enforces the operator's
labels; it does not infer sensitivity.

## 5. Industrial, OT and physical effects

**Action:** change a setpoint, open a valve, dispatch a maintenance job, unlock a
physical access point.
**Failure:** a permitted command is safe alone and unsafe in a sequence or at a
state; a maintenance action proceeds while a safety interlock is active.
**Operator checks:** interlock state; permit-to-work; shift authorization; rate and
duration limits; state preconditions.
**Dominant axes:** authority (interlock/permit state), sequence (safe order), capacity
(rate/duration).
**Evidence must show:** the command, the state consulted, the interlock reading, the
authorizing role, and the time.
**Boundary:** safety integrity is certified equipment's job. The engine records and
refuses; it does not certify safety.

## 6. Agent-to-agent delegation

**Action:** one agent delegates a task or a narrowed capability to another agent.
**Failure:** delegation widens scope; a sub-agent acts after the parent's authority is
revoked; a chain of delegations composes beyond the original grant.
**Operator checks:** delegation attenuation (scope must not widen); revocation state
at dispatch; delegation depth and breadth limits; effect keys across the chain.
**Dominant axes:** authority (revocation), composition (delegation chains), effect
(same action performed by a delegate).
**Evidence must show:** the delegation chain, the attenuations applied, the revocation
state, and the final actor identity.
**Boundary:** identity federation is external. The engine enforces the operator's
attenuation rules.

## 7. Regulated reporting and filings

**Action:** submit a regulatory report, file a disclosure, sign an attestation.
**Failure:** a permitted draft is submitted after the underlying data changed; a
filing is submitted twice; an attestation is signed by an identity that no longer
holds the role.
**Operator checks:** data-freshness threshold; duplicate submission by effect key;
role validity at dispatch; approval quorum.
**Dominant axes:** authority (role validity, data freshness), effect (duplicate
detection), sequence (review before submission).
**Evidence must show:** the artifact hash, the data snapshot identity, the approver(s),
and the submission time.
**Boundary:** legal correctness is not asserted. The engine records what was submitted,
by whom, on what data.

---

## 8. What is common across all seven

| Common property | Why it matters |
|---|---|
| The decision and the effect are separated in time | that gap is the whole problem |
| Authority lives in an operator system, not in the agent | the engine must consult, not assume |
| Individual actions can be permitted while the composition is not | per-request policy is insufficient |
| The evidence must be readable by someone who was not present | audit is a third-party activity |
| The operator, not the vendor, defines domain truth | domain neutrality is what makes it a layer |

## 9. What is *not* common

The checks, the authority sources, the retention rules, the approver population and
the regulatory mapping. A product that ships "the checks" for one domain becomes a
point solution. A layer ships the mechanism and lets the operator express the domain.

That is the difference between a product for payments and an effect-gated execution
layer that is used for payments.
