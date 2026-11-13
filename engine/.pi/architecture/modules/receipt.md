# Receipt and Attestation

<!--
Canonical Reference: .pi/architecture/modules/receipt.md
Implements: ADR-024 §4 (the field set), §5 (workspace identity), §6 (the seal's location), §8 (budget), §10 (one module)
Last Updated: 2026-11-13
Module version: 1.0.0
-->

## Overview

A host that submits an execution declares what it expects to happen; the engine records what it
observed. The two are different types in different directions of trust, and that asymmetry is the
contract: a receipt that lies is **refused**, an attestation that lies **cannot be produced**.

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| `Receipt` | `src/receipt/domain/receipt.rs` | the declaration: steps, declared effects, expectations, workspace, sandbox, roles, budget | #receipt |
| `Attestation` | `src/receipt/domain/attestation.rs` | the observation, signed; private constructor | #attestation |
| `ReceiptRefusal` | `src/receipt/domain/refusal.rs` | the closed refusal vocabulary, and `ReceiptVerdict = Verified \| Refused(..)` | #refusals |
| canonical form | `src/receipt/domain/digest.rs` | one function, one digest — what every seal and signature is taken over | #canonical |
| `verify` / `AttestationBuilder` | `src/receipt/application/service.rs` | verify before dispatch; attest after the last step; the state-directory rule | #verify |

## Receipt

| field | rule |
|---|---|
| `schema_version` | one integer, first in the canonical bytes, unknown ⇒ `SchemaVersion` naming the supported set |
| `workspace` | `{ kind, value }`; `CommitTree` is the only kind 1.15.0 accepts, and an unknown kind is refused |
| `sandbox` | `{ regime: container \| vm, image_digest }` — `none` is **not representable**, so an unisolated run cannot be asked for by omission |
| `steps[].declared_effects` | paths and their content digests, pairwise: a pair that is not a pair is refused, because a comparison that truncated is how an undeclared write passes |
| `steps[].parameters` | scanned at construction; a credential is `SecretInParameters` and the refusal quotes the **path**, never the value |
| `budget` | declared only. `enforced: false` lives on the attestation's echo — nothing here refuses a dispatch |
| `quality_required` | `{ level, source: Caller }`: a receipt is a declaration, so its level is never anything else |

## Attestation

Every field is read from something this execution did. There is no `Default`, no public constructor
and no `Deserialize` for the type — the only path from bytes to a value is
`verify_attestation(bytes, key)`, which checks the HMAC over the canonical body first. A wire mirror
inside the module provides the parsing, so `grep 'impl.*Deserialize.*for Attestation'` finds nothing.

| field | rule |
|---|---|
| `receipt_digest` | echoed from the verified receipt, not restated |
| `steps[]` | observed effects and whether each expectation was met, with the reason when not |
| `quality` | `source: Engine { step_id }` **only** when a step in this execution established a level above the caller's claim; otherwise `source: Caller`, and no field reads as passed |
| `budget` | `{ declared, observed, enforced: false }` |
| `signature` | HMAC-SHA256 over the canonical body, the audit envelope's mechanism; absent when no key is configured (signing is never silently simulated) |

## Verify

1. schema version ∈ supported;
2. sealed digest (when present) = digest of the canonical form;
3. workspace identity = the identity the **engine** computed (never the receipt's own string);
4. sandbox present;
5. every `depends_on` names a step; no cycle; declared effects pairwise;
6. the credential scan — at construction, so nothing downstream copies a secret.

A refusal list is never empty when returned, and a `VerifiedReceipt` is the only way to reach the
`AttestationBuilder`. So "the receipt was refused" and "nothing was dispatched" are one statement.

## The state directory

`default_state_dir` returns `RIGORIX_STATE_DIR` when set, and otherwise a **sibling** of the
repository (`<repo>/../.execution-state/<repo-name>`) — outside the workspace, because the agent's
write tools are contained to `workspace_root` and a seal the agent can rewrite is not a seal.
`refuse_state_dir_inside_workspace` compares resolved paths, so a `sub/../.rigorix/state` spelling is
the same location as `.rigorix/state` and is refused the same way. This is tamper-**evidence**: a
different user with the same path can still read the store, and the ADR says so rather than implying
a permission model that does not exist.

## Tests

`engine/tests/unit/receipt/receipt_verification_test.rs` (19 cases) and
`engine/tests/unit/attestation/attestation_verification_test.rs` (9 cases) assert: each refusal by
variant, the canonical form ignoring member order and not content, the undeclared-write and
digest-mismatch comparisons, the state-directory rule both ways, the budget echo, the two quality
sources, signing and tamper, and the type surface (no `Deserialize`, no `Default`, no `new`).
