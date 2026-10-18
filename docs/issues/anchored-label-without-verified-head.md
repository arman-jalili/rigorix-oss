---
guardian_issue:
  id: "ISSUE-ANCHORED-LABEL-WITHOUT-VERIFIED-HEAD"
  title: "audit: an envelope is labelled HistoryIntegrity::Anchored whenever the anchor is in anchored mode, even when no verified head exists"
  epic: "audit integrity"
  component: "AuditEnvelopeFactory"
  module: "audit"
  status: planned
  priority: high
  dependencies:
    - "ADR-016 — audit integrity anchor"
    - "#899 — anchored mode"

  intent: |
    Separate 'the anchor is configured' from 'this record carries verifiable anchor evidence'.
    `build_envelope` chooses the integrity label from the runtime *mode* and takes the head from
    `runtime.head()`, which is `None` until a history read has been verified. The two are
    decided independently, so an envelope can be stamped `Anchored` with `anchor_head: None`.

    A verifier that trusts the label then treats an unanchored record as anchored. The label is
    the only thing a consumer needs to look at; making it optimistic defeats the point.

  in_scope:
    - "Derive the label from the evidence actually present, not from the configured mode"
    - "When anchored mode is configured but no head is available, label it as degraded/not-anchored"
    - "Keep the configured-mode information separately, so the operator can see the intent"
    - "Test: anchored mode with no verified head must not produce Anchored"
    - "Document what the label asserts"

  out_of_scope:
    - "Changing the anchor verification algorithm"
    - "Anchor availability during startup (the fix is to label honestly, not to block)"
    - "The projection bundle's provenance limits (separate issue)"

  affected_layers:
    domain:
      - "Modified: HistoryIntegrity semantics — label expresses evidence, not configuration"
    application:
      - "Modified: build_envelope — label and head decided together"
    infrastructure:
      - "None"
    api:
      - "Modified: envelope history_integrity value in the degraded case"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-016-audit-integrity-anchor.md"
    - evidence: "engine/src/audit/application/envelope_factory_impl.rs:129-136 — the label comes from runtime.mode(), while anchor_head comes from runtime.head()"
    - evidence: "engine/src/audit/application/envelope_factory_impl.rs:133 — (Some(HistoryIntegrity::Anchored), runtime.head()) — head may be None"
    - evidence: "engine/src/audit/application/envelope_factory_impl.rs:299-300 — the test asserts Anchored together with a head, so the unlucky combination is untested"

  acceptance_criteria:
    - "Anchored is emitted only when an anchor head is present"
    - "A configured-but-headless anchor produces a distinct, honest label"
    - "A test covers anchored mode with no verified head"
    - "The label's meaning is documented where it is defined"
    - "Existing anchored envelopes (label + head) are unchanged on the wire"

  validators:
    - ci
    - tests
    - canonical

  implementation_notes: |
    - The existing test only covers the happy pair (label and head together), which is why the
      mismatch is invisible. Add the negative case.
    - Prefer adding a distinct value or reusing the degraded marker over silently downgrading to
      LocalUnanchored: the operator should be able to tell 'anchored' from 'meant to be anchored,
      could not verify'.
    - If changing the enum is a wire change, check whether any consumer switches on it before
      deciding between a new variant and a nullable field.

  file_changes:
    - "modify: engine/src/audit/application/envelope_factory_impl.rs"
    - "modify: engine/src/audit/domain/envelope.rs"
    - "create: engine/tests/unit/audit/anchored_label_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-ANCHORED-LABEL-WITHOUT-VERIFIED-HEAD

## The defect

```rust
let (history_integrity, anchor_head) = match &self.anchor {
    Some(runtime) if runtime.mode() == AnchorMode::Anchored => {
        (Some(HistoryIntegrity::Anchored), runtime.head())   // head may be None
    }
    _ => (Some(HistoryIntegrity::LocalUnanchored), None),
};
```

The label is decided from **configuration**. The head is decided from **evidence**. They are
independent, so the four-way combination is possible and the wrong pair is reachable:

| mode | head | label | truthful? |
|---|---|---|---|
| anchored | Some | Anchored | yes |
| anchored | **None** | **Anchored** | **no** |
| local | None | LocalUnanchored | yes |

## Why it matters

`HistoryIntegrity` is the field a consumer reads to decide how much to trust the record. If
`Anchored` can mean "the anchor was configured", then the label no longer answers the question
it appears to answer, and a verifier that trusts it accepts an unanchored record.

The failure is quiet and early-deployment shaped: the head is populated after a successful
history read, so this is exactly the window at startup.

## Suggested fix

Decide the label from the evidence:

```rust
let label = match (runtime.mode(), runtime.head()) {
    (AnchorMode::Anchored, Some(_)) => HistoryIntegrity::Anchored,
    (AnchorMode::Anchored, None)    => HistoryIntegrity::AnchoredPending, // distinct, honest
    _                               => HistoryIntegrity::LocalUnanchored,
};
```

Keep the configured intent visible elsewhere; do not let configuration masquerade as evidence.
Add the negative test — the current test only covers the happy pair.
