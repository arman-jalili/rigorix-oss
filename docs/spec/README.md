# Rigorix specifications

The specification: what the gate is, what it must do, and what it does not claim.

There is one specification and one set of requirements. The five gates are sections of that
set — `03-requirements.md` §L — not separate documents: a gate is a decision the engine makes,
and adding a decision adds a section, not a file. Nothing here carries a version; a document is
read at a commit, and its history is git's.

## Index

| Document | What it presents |
|----------|------------------|
| [Execution governance](./execution-governance/README.md) | the numbered set, 00 to 12 — scope, model, requirements (including the five gates, §L), evidence, conformance, non-claims |
| [Validation protocol](./validation-protocol-v1.md) | how the gate's claims are checked, and what a check does not establish |
| [Terminology](./terminology.md) | the normative vocabulary the other documents use |

## How to cite

Name the document, the section, and a commit:

> `docs/spec/execution-governance/03-requirements.md` §L.1, at commit `c8d162e1`

A section anchor is stable; the commit is what makes the citation checkable, because a published
document is not edited in place and its history is the record of what changed.

## Versioning policy

There are no document versions to keep in sync. A change is a commit; a change to jurisdiction,
to a non-claim, or to a failure semantic is a commit that says so, and it is visible in the
history of the section it changed. A published verifier accepts records produced by any earlier
revision of the requirement it implements; a change that alters what a record means is recorded
in the requirement itself, with the trace that shows both sides.

## Claim ceilings (read this before citing)

A specification here bounds a claim; it does not raise it. Claims ascend three
layers and **a lower layer never confers a higher one**:

| Layer | Establishes | Does NOT establish |
|-------|-------------|--------------------|
| L1 engineering verification | bounded behavior under a stated fixture and version | anything about the real world |
| L2 protocol validation | specified properties hold across validated domain mappings | compliance, certification, fitness |
| L3 independent evidence | externally attributable facts | that the system is safe to deploy |

A passing test is **L1** evidence. It does not silently inherit standing as L2 or
L3. See `execution-governance/03-requirements.md` §I.

## Writing a new specification

1. State the jurisdiction and, normatively, the **non-claims**.
2. Give every claimed property a falsifiable case with a stated failure
   condition (see the validation protocol for the shape).
3. Define terms in a normative appendix if any are overloaded.
4. Classify the specification's evidence layer honestly.
5. Put it where its siblings are: a requirement goes in `03-requirements.md`, beside the
   requirement it constrains — a new document is for a new subject, never for a new section.
