---
guardian_issue:
  id: "ISSUE-PHASEC-7-SPEC-RECORDS-WHAT-C-MADE-TRUE"
  title: "Phase C.7 — the specification records what Phase C made true: the determinism witness behind RS-004, the producer-only deployment mode, who supplies the decision instant, and refused-versus-could-not-evaluate"
  epic: "portable-slice"
  component: "Spec"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1387
  dependencies:
    - "Phase C.5 — conformance produces CLI-labelled evidence (must be merged)"
    - "Phase C.4 — the crate refuses with `gate_unavailable` only when it cannot evaluate (must be merged)"

  intent: |
    The specification is normative and its version is published. Four statements in it are now
    demonstrable where they were previously asserted — and one obligation it already makes is now
    applied to a second deployment mode. None of this is reflected, so the spec describes a world in
    which the product's strongest claim is a design intention.

    The four edits, each anchored in existing text:

    1. **RS-004** — "A plan MUST be rejected if it cannot be evaluated deterministically"
       (`03-requirements.md:28`). Its traceability row (`11-traceability.md:13`) cites "fail-closed
       evaluation in sequence/precondition modules | negative tests". After C.5 the evidence is
       stronger: a golden digest produced by two runtimes from one composition, with `gate: "cli"`
       vectors. The row must cite ADR-014 §2, the golden test and the conformance label, or the spec
       understates what is proven.

    2. **Deployment mode** — `00-scope.md:109` already says "A deployment MUST NOT claim a level whose
       underlying mode is absent", and `09-conformance.md:7` requires a claim "in a declared
       deployment mode, with stated exceptions". A wasm producer that ships no verifier (ADR-013) is
       exactly such a mode: it produces signed evidence and cannot perform offline verification, so it
       MUST NOT claim Level 4 on its own. Today no document says so, and the browser demo is the
       deployment that most needs the sentence.

    3. **Who supplies the decision instant** — `05-evidence-model.md:20` lists the decision record as
       "rule id, step, outcome, inputs consulted, digests, time". With pinned inputs (ADR-014 §2) the
       instant may be supplied by the caller and is signed. The spec must state that the record's
       `time` is the instant the decision was evaluated with, and that a verifier replays with the
       recorded instant — never with the wall clock at verification time. Without that sentence,
       "reproducible decision" has no normative content.

    4. **Refused versus could-not-evaluate** — `01-terminology.md:156` says of a precondition:
       "Failure, absence, timeout, unparseable output and unknown all refuse." That is already the
       rule the wasm vocabulary now obeys. What is missing is the obligation that the *evidence* keeps
       the two apart: a refusal is a decision, an unavailable evaluation is not, and a host must not
       be able to read the second as the first. This is the normative home of `gate_unavailable`.

  in_scope:
    - "The four edits above, each citing the artifact that demonstrates it (ADR-014 §2, the golden test, the `gate: \"cli\"` vectors, ADR-013)"
    - "`11-traceability.md`: the RS-004 row and any row whose evidence is now a command rather than a test name"
    - "`12-versioning.md` compliance: a **minor** bump 0.2.0 → 0.3.0, with the rationale, the trace and the conformance impact recorded as that file requires"
    - "The spec README's version line and the document index, so the published copy and the source agree"

  out_of_scope:
    - "Any new RS requirement, any stricter obligation, or any new conformance level. Those are **major** per `12-versioning.md:27-31`; if an edit turns out to need one, stop and file the delta for review rather than bumping to 1.0.0 unilaterally"
    - "Re-publishing the spec site in `rigorix-gtm`. The published copy must be re-synced in the same release and its claim-coverage check re-run — that is a separate repository and a separate PR, noted as a release task, not executed here"
    - "Phase C's code. This issue records evidence, it does not produce any"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "None"
    interfaces:
      - "The normative text itself: 00-scope, 01-terminology, 05-evidence-model, 09-conformance, 11-traceability, 12-versioning, README"

  citations:
    - file: "docs/spec/execution-governance/03-requirements.md"
      note: ":28 RS-004, :79 RS-021 (unreachable/timeout/unknown refuse)"
    - file: "docs/spec/execution-governance/11-traceability.md"
      note: ":13 the RS-004 row this issue strengthens"
    - file: "docs/spec/execution-governance/00-scope.md"
      note: ":89 a requirement may be conditional on a declared capability or deployment mode; :109 'MUST NOT claim a level whose underlying mode is absent'"
    - file: "docs/spec/execution-governance/05-evidence-model.md"
      note: ":20 the decision record's fields, including `time`"
    - file: "docs/spec/execution-governance/01-terminology.md"
      note: ":156 'Failure, absence, timeout, unparseable output and unknown all refuse'"
    - file: "docs/spec/execution-governance/12-versioning.md"
      note: ":27-31 new/stricter requirement is major; :9 the version form"
    - file: "wasm/.pi/architecture/decisions/ADR-013-who-verifies.md"
      note: "the crate produces, the host verifies — the basis for the producer-only mode"

  verification:
    - "RS-004's traceability row names ADR-014 §2, the golden test by name, and `gate: \"cli\"`; a reader can run one command and see the witness"
    - "The declared deployment mode appears in `09-conformance.md` §1 or §6 with its exception stated, and states that Level 4 is not claimed by a producer that ships no verifier: `grep -n 'producer' 09-conformance.md` shows the rule"
    - "`05-evidence-model.md` says the record's `time` is the instant used for evaluation and that a verifier replays with it — `grep -n 'replay' 05-evidence-model.md` returns at least one normative sentence"
    - "`01-terminology.md` distinguishes a refusal from an unavailable evaluation, and the wasm crate's refusal table cites it"
    - "`grep -c '0.2.0' docs/spec/execution-governance/README.md` returns 0 and the version reads 0.3.0, with `12-versioning.md`'s rationale, trace and conformance-impact fields filled"
    - "No RS identifier was added or renumbered: `git diff docs/spec/execution-governance/03-requirements.md` shows no new `RS-` line (a new one is a major bump and stops this issue)"
---
**This issue is C.7 (#1387). Prerequisite: C.4 (#1384) and C.5 (#1385).** Do not begin work before every
prerequisite listed above is *merged* — a locally patched engine or a stubbed crate makes
this issue's acceptance criteria untestable.

**The Phase C chain, in order:**

```
C.1 #1381  engine gate entry point        (no prerequisite)
C.2 #1382  CLI adopts it                 needs C.1
C.3 #1383  crate wires EngineGate        needs C.1, C.2   <- do not start before both are merged
C.4 #1384  gate_unavailable narrowed     needs C.3
C.5 #1385  conformance -> gate: "cli"    needs C.3, C.4
C.6 #1386  size budget re-recorded       needs C.3, C.4, C.5
C.7 #1387  spec records the evidence     needs C.4, C.5
```


# Before you start

**C.4 and C.5 must be merged.** The spec must cite evidence that exists: a `gate: "cli"` vector and a
`gate_unavailable` that only means "this build cannot evaluate". Citing a planned artifact is how the
report ended up with a digest from a different tree (#1357).

# Why this is a Phase C issue and not an afterthought

Three of the four edits are clarifications of obligations the spec already makes. The fourth — the
deployment mode — is the one that decides what the browser demo may claim, and it is the sentence a
buyer or a sceptical reader will look for. Leaving it out means the demo either claims nothing
(useless) or claims Level 4 from a component that deliberately cannot verify (false).
