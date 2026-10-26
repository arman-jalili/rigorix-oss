# Rigorix specifications

Versioned, citable specifications for Rigorix behavior. Unlike an ADR (an
internal decision record), a document here has a **stable identifier**, a
**version**, and a **date**, so a public claim can be cited against a fixed
artifact and checked against stated failure conditions.

## Index

| Specification | Version | Date | Status |
|---------------|---------|------|--------|
| [Consequence Gating](./consequence-gating-v1.md) | 1.0 | 2026-10-12 | Published |
| [Consequence Gating — Validation Protocol](./validation-protocol-v1.md) | 1.0 | 2026-10-12 | Published |
| [Terminology](./terminology.md) | 1.0 | 2026-10-12 | Published |
| [Effect-Gated Execution](./effect-gated-execution/README.md) | 0.2 | 2026-10-25 | Review draft |

## How to cite

Name the identifier, the version, and a section anchor:

> Rigorix Consequence Gating v1.0 §2.7 (non-inheritance)

Identifiers are namespace-stable:

- `rigorix-consequence-gating`
- `rigorix-consequence-gating-protocol`
- `rigorix-consequence-gating-terminology`
- `rigorix-effect-gated-execution` (14 documents; requirement identifiers `EGE-NNN`)

A review draft may be cited for review and disagreement; no conformance claim may
cite it. See the document's own §Status.

> Rigorix Effect-Gated Execution v0.2 §A (plan and scope)

## Versioning policy

- **Minor** — an additive change that alters no stated property (for example a
  new optional evidence field).
- **Major** — a change to jurisdiction, to a non-claim, or to a failure semantic
  requires a new major version and a new document (a published version is not
  edited in place).
- Published verifiers accept records produced by any `1.x` version; a record
  produced under a later **major** version MAY be rejected by an earlier verifier.

## Claim ceilings (read this before citing)

A specification here bounds a claim; it does not raise it. Claims ascend three
layers and **a lower layer never confers a higher one**:

| Layer | Establishes | Does NOT establish |
|-------|-------------|--------------------|
| L1 engineering verification | bounded behavior under a stated fixture and version | anything about the real world |
| L2 protocol validation | specified properties hold across validated domain mappings | compliance, certification, fitness |
| L3 independent evidence | externally attributable facts | that the system is safe to deploy |

A passing test is **L1** evidence. It does not silently inherit standing as L2 or
L3. See `consequence-gating-v1.md` §6.

## Writing a new specification

1. State the jurisdiction and, normatively, the **non-claims**.
2. Give every claimed property a falsifiable case with a stated failure
   condition (see the validation protocol for the shape).
3. Define terms in a normative appendix if any are overloaded.
4. Classify the specification's evidence layer honestly.
5. Give it a stable identifier, a version, and a date.
