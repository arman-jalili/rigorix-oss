---
guardian_issue:
  id: "ISSUE-PROJECTION-BUNDLE-PROVES-CONSISTENCY-NOT-PROVENANCE"
  title: "audit: verify_authority_fact_bundle proves the projection matches the supplied history, not that the history came from the anchor"
  epic: "audit integrity"
  component: "AuthorityFactProjection"
  module: "audit"
  status: planned
  priority: high
  dependencies:
    - "ADR-016 — audit integrity anchor"
    - "#1001 — authority-fact projection export"

  intent: |
    Make the offline verifier's claim match what it establishes. `verify_authority_fact_bundle`
    checks that the bundled history is an internally consistent chain, re-derives the facts with
    the same pure function, and compares. That is a real property: it catches a signer asserting
    facts the supplied history does not support — something a signature over the projection
    cannot catch.

    What it does not establish: that the supplied history is authentic, was ever recorded by an
    external anchor, or is attributable to the claimed producer. `verify_chain` links hashes; it
    does not verify an anchor signature, and it does not verify envelope HMACs against a trusted
    key. A third party can construct a self-consistent chain and a matching projection without
    ever having obtained the real history.

  in_scope:
    - "Include an anchor-signed checkpoint (or signed history slice) in the bundle"
    - "Verify the anchor signature offline against a supplied trusted public key"
    - "Optionally verify envelope HMACs against a supplied trusted key"
    - "Return distinct outcomes: chain-consistent vs anchor-authenticated vs neither"
    - "Name the verified property in the API and in the doc comment"
    - "If the signature is out of scope, narrow the claim explicitly"

  out_of_scope:
    - "Key distribution or trust-on-first-use policy"
    - "The anchored-label defect in the envelope factory (separate issue)"
    - "Rewriting the projection derivation"

  affected_layers:
    domain:
      - "Modified: AuthorityFactBundle — carries anchor proof"
      - "Modified: verification outcome — distinguishes consistency from provenance"
    application:
      - "Modified: verify_authority_fact_bundle — verify the anchor proof"
    infrastructure:
      - "Modified: export path — embed the proof"
    api:
      - "Modified: CLI/MCP projection verify surface — reports which property holds"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-016-audit-integrity-anchor.md"
    - evidence: "engine/src/audit/application/projection.rs:161 — verify_authority_fact_bundle: verify_chain, then re-derive, then compare"
    - evidence: "engine/src/audit/application/projection.rs:163-168 — the doc says 'history that must first be an intact chain'; intact is not authenticated"
    - evidence: "engine/src/audit/application/chain.rs:63 — verify_chain groups by producer and checks sequence; no signature or HMAC verification"
    - evidence: "engine/src/audit/application/projection.rs:180 — only facts and anchor_ref are compared; anchor_ref is derived from the bundle's own envelopes"

  acceptance_criteria:
    - "The verifier states which property it established"
    - "A bundle with a tampered-but-self-consistent history is rejected, or the limitation is documented as the claim ceiling"
    - "Anchor-authenticated verification is possible offline with only public material"
    - "The API result distinguishes chain-consistent from anchor-authenticated"
    - "Tests: forged self-consistent chain, valid chain with bad anchor signature, valid bundle"

  validators:
    - ci
    - tests
    - canonical
    - security

  implementation_notes: |
    - Note carefully: `anchor_ref` is derived from the bundle's own envelopes (`anchor_ref(&envelopes)`),
      so comparing it to the projection compares the bundle to itself. It detects an inconsistent
      exporter, not a forged history. Say that plainly in the doc.
    - The anchor client already verifies an Ed25519 signature on a history slice before handing it
      to the matcher — that machinery exists. The gap is that the offline bundle does not carry
      the evidence needed to redo it.
    - Narrowing the claim is an acceptable resolution if the signed-checkpoint work is larger than
      it looks. The defect is the claim, not the absence of the feature. But the current wording
      ('the anchored history') implies provenance that is not established.

  file_changes:
    - "modify: engine/src/audit/application/projection.rs"
    - "modify: engine/src/audit/domain/authority_fact.rs"
    - "create: engine/tests/projection_provenance.rs"
    - "modify: docs (projection verification claim)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-PROJECTION-BUNDLE-PROVES-CONSISTENCY-NOT-PROVENANCE

## What the verifier does

```rust
pub fn verify_authority_fact_bundle(bundle: &AuthorityFactBundle) -> Result<(), ...> {
    verify_chain(&bundle.envelopes)?;                       // internal linkage
    let derived = ProjectAuthorityFacts::project(&bundle.envelopes, ...);
    if derived.facts != bundle.projection.facts { ... }     // self-consistency
    if derived.anchor_ref != bundle.projection.anchor_ref { ... }
    Ok(())
}
```

## What that establishes

> The projection is consistent with the supplied history, and certain tampering with that
> history is detectable.

That is genuinely useful — it catches a signer asserting facts the history does not support,
which a signature over the projection cannot catch.

## What it does not establish

> That the supplied history is authentic, externally anchored, or attributable to the claimed
> producer.

`verify_chain` (`chain.rs:63`) groups envelopes by producer and checks sequence continuity. It
does **not** verify an anchor signature and does **not** verify envelope HMACs against a trusted
key. So the chain is only as trustworthy as the person who handed it over.

A third party can build a self-consistent chain and a matching projection without ever having
obtained the real history, and this verifier accepts it.

## One subtlety worth stating

`anchor_ref` is derived **from the bundle's own envelopes**:

```rust
fn anchor_ref(envelopes: &[AuditEnvelope]) -> Option<AnchorRef>
```

So step 3 compares the bundle to itself. It detects an *inconsistent exporter*, not a *forged
history*. The doc's phrase "the anchored history" implies the latter.

## Suggested fix

Either:

- **Strengthen** — carry an anchor-signed checkpoint in the bundle, verify it offline against a
  supplied trusted public key, and return a result that distinguishes *chain-consistent* from
  *anchor-authenticated*. The anchor client already verifies Ed25519 signatures on a history
  slice; the bundle just does not carry the evidence to repeat it.

- **Or narrow** — rename the outcome to `consistency`, and state in the doc and the CLI output
  that provenance requires a separate trusted-anchor check.

Either is defensible. Leaving the current wording is not: it lets a self-consistency check be
read as independent verification.
