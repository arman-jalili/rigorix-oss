---
guardian_issue:
  id: "ISSUE-DOC-CLAIMS-WIRING-AUDIT"
  title: "audit: verify every module doc and ADR claim of end-to-end wiring against the call graph, then fix or delete what does not hold"
  epic: "docs hygiene"
  component: "Docs"
  module: "governance"
  status: planned
  priority: high
  dependencies:
    - "#1072 — closed COMPLETED while the oracle still ships disabled"
    - "#1096 — the scan that could not see backticked paths"

  intent: |
    Module docs and ADRs assert that things are wired, enforced, or implemented end to end. Those
    assertions are load-bearing: a reader decides whether a control protects them from a doc, not from
    a call graph. This audit checks every such claim against code and either confirms it, files the
    gap, or deletes the claim. No claim is softened to "planned" without a filed issue saying so.

    The motivation is not hypothetical. #1072 was closed COMPLETED while its title — "disabled in
    every production entry point" — remained true. Nothing in the process contradicted it, because the
    doc was checked by reading it.

  in_scope:
    - "Every file under */.pi/architecture/modules/*.md and */.pi/architecture/decisions/ADR-*.md"
    - "Every sentence asserting wiring, enforcement, reachability, or end-to-end implementation"
    - "Classify each as CONFIRMED (with the query that proves it), GAP (filed), or DELETE (claim is false)"
    - "Prefer a call-graph query over grep: reachability is a graph property"
    - "Produce one table per document, with the evidence, committed under docs/audit/"

  out_of_scope:
    - "Style edits, restructuring, or merging documents"
    - "Claims about intent, rationale, or history — only claims about what the code does"
    - "Inventing wiring to satisfy a doc (the doc changes, not the code, unless a gap is filed)"

  affected_layers:
    domain:
      - "All module docs"
    application:
      - "All ADRs"
    infrastructure:
      - "docs/audit/ (new)"
    api:
      - "None"

  canonical_references:
    - evidence: "#1072 — CLOSED COMPLETED; effect_scope.rs:24 still says 'Not wired by default ... no effect-scope coverage is produced in the OSS default'"
    - evidence: "engine/.pi/architecture/modules/*.md — 30+ module docs, each with a Wiring or Components section"
    - evidence: "engine/.pi/architecture/decisions/ADR-*.md — 18+ ADRs asserting implementation status"
    - evidence: "the existing guard (validate-docs.sh) checks links, changelog and identifiers; it does not check whether 'wired' is true"

  acceptance_criteria:
    - "Every module doc and ADR has an audit row per wiring claim: CONFIRMED / GAP / DELETE"
    - "CONFIRMED rows cite a call-graph query (gitnexus_impact or equivalent), not a grep"
    - "Every GAP has a filed issue number; every DELETE has the sentence removed"
    - "A repeatable script re-runs the audit and reports claims whose evidence has disappeared"
    - "Zero documents assert wiring that the graph cannot confirm"

  validators:
    - canonical
    - architecture
    - ci

  implementation_notes: |
    Method that actually works, learned the hard way in this repo:

    1. Extract claims mechanically — sentences containing wired, wired in, enforced, enforced at,
       reachable, called from, end to end, implemented, in every entry point.
    2. Resolve each to a symbol and run the call graph, not a text search. Grep finds a name; the
       graph finds whether anything reaches it. #1072 was invisible to grep because the code was
       present and correct — it simply had no caller.
    3. Treat a claim with no in-scope caller as a GAP even when the code is perfect. The defect
       class is "exists but unreached", and it is the one this repo keeps producing.
    4. Check the closure discipline too: an issue closed COMPLETED must not leave a doc asserting the
       half that was deferred.

    The audit is not a one-off read-through. Its durable artifact is the script plus the table, so the
    next regression shows up as a stale evidence row rather than as something a reviewer has to
    remember to look for.

    Known instances to seed the audit with (each already verified):
    - effect_scope.rs: wired in no entry point (#1072 closed COMPLETED)
    - the drain path: correctly wired, but its only regression test drained an empty queue
    - .pi/ path references: two wrong after #1096, one more found in #1096's wake

  file_changes:
    - "create: docs/audit/wiring-claims.md"
    - "create: .pi/scripts/validate-wiring-claims.sh"
    - "modify: module docs and ADRs where a claim is GAP or DELETE"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-DOC-CLAIMS-WIRING-AUDIT

## Why this exists

This series produced a repeating failure: **code that exists, compiles, passes its own tests, and is
never reached.**

| instance | how it was found |
|---|---|
| capacity ledger entirely unwired | the module doc had no EnforcementSeam component, so no wiring was ever planned |
| effect-scope oracle disabled in all 3 roots | reading the module doc against the entry points; #1072 was then closed COMPLETED anyway |
| drain shared no controls | a second ingestion path that owned none of the pipeline |
| drain backoff never persisted | the local mutation existed, the store write did not |

None of these is a compile error, a test failure, or a type mismatch. Each is a **claim of wiring that
was true of the code in isolation and false of the system.**

## The check that would have caught them

| method | finds | misses |
|---|---|---|
| grep for a symbol | the code exists | that nothing calls it |
| test suite | the tested path | an untested sibling path |
| reading the doc | what the doc says | whether it is true |
| **call graph, per claim** | **whether anything reaches it** | nothing relevant here |

Reachability is a graph property. The graph is the only tool in this repo that answers it, and it was
not part of any documentation review.

## Deliverable

A table per document:

| doc | claim | verdict | evidence |
|---|---|---|---|
| \`modules/capacity-ledger.md\` | "reserve precedes dispatch" | CONFIRMED | \`impact(capacity_verdict, upstream)\` → dispatch.rs:229 |
| \`modules/approval.md\` | "effect scope is verified" | **GAP** | no entry point sets \`approval_repo_path\` (#…) |
| \`ADR-0xx\` | "implemented end to end" | **DELETE** | claim removed; half was deferred |

Plus \`.pi/scripts/validate-wiring-claims.sh\`, which re-resolves each CONFIRMED row and fails when
the evidence disappears — so the audit decays visibly instead of silently.
