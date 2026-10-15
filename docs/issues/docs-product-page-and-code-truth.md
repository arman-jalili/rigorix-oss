---
guardian_issue:
  id: "ISSUE-DOCS-PRODUCT-PAGE-AND-CODE-TRUTH"
  title: "Documentation hygiene: the README must convert, and every doc must be verified against code"
  epic: "adoption / documentation"
  component: "Docs"
  module: "documentation"
  status: planned
  priority: high
  dependencies:
    - "v1.10.0 (the surfaces the README must describe are now released)"

  intent: |
    The repository is the primary conversion surface and the README does not carry
    its weight: it leads with technology instead of the problem, has no single-line
    value proposition, no visual, no ecosystem trust signals, and it omits the
    flagship capability that shipped in 1.10.0. Separately, the repo's documentation
    has drifted from the code, and several dated, one-off documents have no ongoing
    value and should go.

    The governing rule for every change here: **the code is the only source of
    truth.** No statement in a doc is kept because it sounds good; it is kept
    because it can be traced to behaviour, and it states what is true now.

  in_scope:
    - "README: problem-first headline + one-line value proposition"
    - "README: a visual of a refusal scenario (GIF or short video)"
    - "README: trust signals — shields.io badges for the verifier packages (crates.io, npm, PyPI), build status, license"
    - "README: describe the 1.10.0 capability (consequence gating / authority-fact projection / re-derivation)"
    - "README: correct every factual claim against code"
    - "Per-crate READMEs (engine, mcp, cli, actions): verified against code"
    - "Root docs (WORKFLOW, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT): verified"
    - "Remove dated one-off documents with no ongoing value"
    - "Explain each capability in BOTH registers: technical (what it does, how it is verified) and business (who needs it, what risk it removes)"

  out_of_scope:
    - "docs/spec/* (versioned specifications) — governed by their own claim-ceiling rules"
    - "The Guardian-side vendored validator scripts (/Users/arman/.bun/bin/guardian-framework) — handled separately"
    - "docs/runbook.md, docs/dr-plan.md, docs/security-guards.md — referenced by code/scripts and must stay"
    - "Writing a new feature to make a claim true: a claim that cannot be traced to code is DELETED, not implemented"

  affected_layers:
    domain:
      - "No code change"
    application:
      - "No code change"
    infrastructure:
      - "No code change"
    docs:
      - "Modified: README.md (the significant one)"
      - "Modified: engine/README.md, mcp/README.md, cli/README.md, actions/README.md"
      - "Modified: WORKFLOW.md, CONTRIBUTING.md (if stale)"
      - "Deleted: dated one-off documents (list below, subject to confirmation)"

  canonical_references:
    - module: "docs/spec/consequence-gating-v1.md (the capability to describe)"
    - module: "docs/spec/README.md (claim ceilings — do not overclaim beyond these)"
    - module: "mcp/src/host/mod.rs (the tool list — the source of the tool count)"
    - module: ".pi/scripts/local-ci.sh (the source of any CI-step figure)"
    - module: ".github/workflows/ci.yml (the job set)"
    - last_sync: "2026-10-14"

  acceptance_criteria:
    - "The README headline leads with the problem, not the technology"
    - "One line states who it is for and why it matters"
    - "A visual (GIF/video) shows a refusal scenario — not a paragraph describing one"
    - "Badges cover crates.io + npm + PyPI verifier packages, build status and license"
    - "Every factual figure in the README is traceable to code and is CURRENT (tool count, CI figures, module counts, versions)"
    - "The 1.10.0 capability is described in both technical and business registers"
    - "Per-crate READMEs contain no stale version or count"
    - "No doc asserts a capability the code does not implement"
    - "Dated one-off documents are removed, with the survivors recorded and justified"
    - "No remaining doc reference points at a deleted file"

  validators:
    - canonical
    - docs
    - ci

  implementation_notes: |
    ## Why this is high priority

    The README is the first thing a security or platform lead reads, and it is the
    only artifact most of them will read before deciding. Right now it undersells
    the product and simultaneously contains claims that are wrong — the worst
    combination, because the wrong parts are checkable.

    ## Measured drift (verified 2026-10-14 against code)

    README.md:

    | claim in README | reality | source of truth |
    |---|---|---|
    | "exposing 14 built-in tools" | **15** OSS descriptors, plus 3 auth tools the README omits entirely (18 ungoverned-visible) | `mcp/tests/schema_descriptor_tests.rs` (15), `mcp/src/auth/interfaces/mcp/mod.rs` (3) |
    | the tool list | missing `rigorix_audit_authority_facts`, `rigorix_auth_login/logout/status` | `mcp/src/host/mod.rs` |
    | "113 automated verification steps" + stage breakdown (Lint 18 / Build 13 / Test 57 / Security 10 / Docs 12 / Integration 2 / Coverage 1) | **not derivable**: `local-ci.sh --list` discovers 25 root + 87 engine + 28 cli + 40 actions scripts; `ci.yml` has 10 real jobs | `.pi/scripts/local-ci.sh --list` |
    | mcp = "6 bounded-context modules" | 9 dirs under `mcp/src` | `find mcp/src -maxdepth 1 -type d` |
    | actions = "9 bounded-context modules" | 10 dirs under `actions/src` | `find actions/src -maxdepth 1 -type d` |
    | coverage "gated >=60%" | **correct** | `.pi/scripts/coverage.sh` (`COVERAGE_THRESHOLD=60`) |
    | demo clone + `node .rigorix/run-demo.mjs` | **correct**: repo is public and the file exists | `arman-jalili/payments-demo` |

    **Missing entirely: the 1.10.0 capability.** The README's "problem" section is
    about agent loops in general. The differentiator that actually shipped — a
    precondition check whose refusal is recorded in a signed envelope, and an
    authority-fact projection a third party verifies **by re-derivation rather than
    by trusting a signature** — appears nowhere. The comparison table still says
    "HMAC-signed envelopes", predating anchors, chain verification and attribution.

    Per-crate READMEs are older than the README and have their own drift:

    | file | last touched | stale example |
    |---|---|---|
    | `engine/README.md` | 2026-07-21 | predates ADR-017 entirely |
    | `cli/README.md` | 2026-06-21 | predates the `audit` subcommands |
    | `actions/README.md` | 2026-06-27 | says `version = "1.0.0"`; actions is 1.4.0 |
    | `mcp/README.md` | 2026-09-01 | says "14 built-in OSS tools" |

    ## The README as a product page

    Apply this order — each element earns the next:

    1. **Headline = the problem.** Lead with what breaks, not what it is built with.
       The failure mode to name is agents composing individually-permitted actions
       into an outcome no one authorised. Candidate phrasing to work from, not to
       copy verbatim: "Stop AI agents from composing individual actions into
       harmful outcomes."
    2. **One-line value proposition = who + why.** Name the buyer and the setting:
       security and platform teams governing autonomous agents in regulated
       money-out, CI/CD, and clinical workflows.
    3. **Visual proof before prose.** A GIF or short clip of one refusal: the action
       is attempted, the gate refuses, the record is written. The current text says
       "watch it stop" and then shows a terminal block — that is a description of a
       visual, not a visual.
    4. **Trust signals.** shields.io badges for the verifier packages on crates.io,
       npm and PyPI, plus build status and license. One hand-written crates badge
       for `rigorix-cli` is not an ecosystem signal; the verifier packages published
       in four languages are.
    5. **Then** architecture, templates, quickstart, comparison.

    ## Both registers, for each capability

    Every capability needs two paragraphs, and they must not be blended:

    - **technical** — what the mechanism does, and how a third party can check it
      (e.g. re-derivation from the anchored chain, the claim ceiling, NC-4);
    - **business** — who needs it and what risk it removes (e.g. an auditor who
      needs to know what an agent was permitted to do, without trusting the vendor).

    The claim-ceiling rules in `docs/spec/README.md` bound the business register:
    engineering verification (L1) must not be written as protocol validation (L2) or
    independent evidence (L3).

    ## Removal candidates — dated one-off documents

    Measured reference counts from code/scripts (0 = nothing depends on it):

    | document | code refs | disposition |
    |---|---|---|
    | `docs/next-workstreams-2026-09-21.md` | 0 | remove (dated planning) |
    | `docs/contract-conformance-2026-09-08.md` | 0 | remove (dated snapshot) |
    | `docs/sdk-server-strategy-freeze-2026-09-06.md` | 0 | remove (dated strategy snapshot) |
    | `docs/panic-audit-2026-10-12.md` | 0 | **decide**: it documents the panic baseline the guard enforces; consider folding the durable parts into `docs/security-guards.md` and removing the dated report |
    | `docs/runbook.md` | 10 | **keep** |
    | `docs/dr-plan.md` | 5 | **keep** |
    | `docs/security-guards.md` | 1 | **keep** |

    Also decide `docs/issues/*` (11 files): they mirror Guardian issues, are not
    synced by any script, and several describe issues that are now CLOSED. They are
    historical work records, not product documentation — argue for keeping or
    removing explicitly. Do not delete them as a side effect of this issue.

    ## Verification method

    Every figure in every doc must be re-derived from code before it is written. Two
    rules:

    1. If a number cannot be derived, replace it with a derivable one or remove it.
       Do not estimate.
    2. If a claim cannot be traced to behaviour, delete the claim. It is not a
       backlog item.

    ## Suggested added guard (optional, but it is the durable fix)

    A `validate-docs.sh` that fails when a doc references a path that does not
    exist, or when a doc pins a version that no longer matches the manifest. The
    drift recurs because nothing checks it; the guard is what stops the next
    round.

  file_changes:
    - "modify: README.md"
    - "modify: engine/README.md, mcp/README.md, cli/README.md, actions/README.md"
    - "modify: WORKFLOW.md, CONTRIBUTING.md (only where stale)"
    - "delete: docs/next-workstreams-2026-09-21.md, docs/contract-conformance-2026-09-08.md, docs/sdk-server-strategy-freeze-2026-09-06.md (pending confirmation)"
    - "create: assets/ (demo GIF/video) referenced from the README"
    - "create (optional): .pi/scripts/validate-docs.sh"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-DOCS-PRODUCT-PAGE-AND-CODE-TRUTH

## Intent

Two problems with one root cause. The README is the primary conversion surface and
it does not convert — it leads with technology, has no value proposition, no
visual, no ecosystem badges, and it never mentions the capability that shipped in
1.10.0. And the documentation as a whole has drifted from the code.

**The rule for all of it: the code is the only source of truth.**

## Dependencies

```
┌────────────────────────────────────────┐
│ v1.10.0                                │  RELEASED
│ the surfaces the docs must describe    │  (CLI projection, MCP tool, providers)
└──────────────────┬─────────────────────┘
                   ▼
┌────────────────────────────────────────┐
│ THIS ISSUE                             │
│ README as product page                 │
│ + every doc verified against code      │
│ + dated artifacts removed              │
└────────────────────────────────────────┘
```

## Workstream 1 — the README is a product page

Order matters; each element earns the next.

| # | element | current state |
|---|---|---|
| 1 | **Headline = the problem** | leads with technology ("The LLM generates code. Rigorix governs execution.") |
| 2 | **One-line value prop (who + why)** | absent; "Built for platform and security teams" is buried after three paragraphs |
| 3 | **Visual of a refusal** | absent — text says "Watch it stop", then shows a terminal block |
| 4 | **Trust signals** | one hand-written crates badge for `rigorix-cli`; no verifier badges |
| 5 | **Then** architecture, templates, quickstart | present and reasonable |

## Workstream 2 — every doc verified against code

### Measured drift

| README claim | reality | source |
|---|---|---|
| "14 built-in tools" | **15** OSS + 3 auth (README omits the auth trio and the new tool) | `mcp/tests/schema_descriptor_tests.rs`, `mcp/src/host/mod.rs` |
| "113 automated verification steps" + stage breakdown | not derivable — `local-ci.sh --list` finds 25 root + 87 engine + 28 cli + 40 actions; `ci.yml` has 10 jobs | `.pi/scripts/local-ci.sh --list` |
| mcp "6 bounded-context modules" | 9 | `mcp/src` |
| actions "9 bounded-context modules" | 10 | `actions/src` |
| coverage "gated ≥60%" | ✅ correct | `.pi/scripts/coverage.sh` |
| demo clone + `run-demo.mjs` | ✅ correct | `arman-jalili/payments-demo` |

### Missing entirely: the 1.10.0 capability

The README never describes what actually differentiates Rigorix. It should, in both
registers:

- **technical** — a precondition is checked outside the agent-writable workspace;
  a refusal is recorded in a signed envelope; an authority-fact projection can be
  verified by a third party **by re-derivation**, not by trusting the producer's
  signature; the artifact never asserts present standing (NC-4).
- **business** — an auditor can establish what an agent was permitted to do, and
  prove it, without trusting the vendor.

The comparison table still reads "HMAC-signed envelopes", predating anchors, chain
verification and attribution.

### Per-crate READMEs are older still

| file | last touched | stale example |
|---|---|---|
| `engine/README.md` | 2026-07-21 | predates ADR-017 |
| `cli/README.md` | 2026-06-21 | predates the `audit` subcommands |
| `actions/README.md` | 2026-06-27 | says `version = "1.0.0"` (actions is 1.4.0) |
| `mcp/README.md` | 2026-09-01 | "14 built-in OSS tools" |

## Workstream 3 — remove dated one-off documents

Reference counts measured from code and scripts:

| document | code refs | disposition |
|---|---|---|
| `docs/next-workstreams-2026-09-21.md` | 0 | remove |
| `docs/contract-conformance-2026-09-08.md` | 0 | remove |
| `docs/sdk-server-strategy-freeze-2026-09-06.md` | 0 | remove |
| `docs/panic-audit-2026-10-12.md` | 0 | decide — it backs the panic baseline; consider folding the durable parts into `docs/security-guards.md` |
| `docs/runbook.md` | 10 | **keep** |
| `docs/dr-plan.md` | 5 | **keep** |
| `docs/security-guards.md` | 1 | **keep** |

`docs/issues/*` (11 files) is a separate decision — historical work records for
issues, several now closed. Argue it; do not delete as a side effect.

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Headline leads with the problem | review |
| 2 | One line states who it is for and why | review |
| 3 | A visual shows a refusal scenario | review |
| 4 | Badges: crates.io + npm + PyPI verifiers, build, license | review |
| 5 | Every README figure derivable from code and current | `validate-canonical.sh` |
| 6 | The 1.10.0 capability described technically **and** commercially | review |
| 7 | Per-crate READMEs carry no stale version/count | `validate-ci.sh` |
| 8 | No doc asserts an unimplemented capability | review |
| 9 | Dated docs removed; survivors recorded with reasons | review |
| 10 | No doc points at a deleted file | `validate-docs.sh` (new, optional) |

## Implementation

> **Agent:** Do **not** write a feature to make a claim true. If a claim cannot be
> traced to code, delete the claim — that is the whole point of this issue. Derive
> every number; never estimate one.

### Steps

1. Derive the real figures (tool count + auth tools; CI shape from `ci.yml` and `local-ci.sh --list`; module counts) — do not reuse the numbers in this issue without re-measuring
2. Rewrite the README top: problem headline → value prop → visual → badges
3. Add the 1.10.0 capability in both registers, bounded by the claim ceilings
4. Sweep every remaining README claim against code; delete what cannot be traced
5. Bring the four per-crate READMEs current
6. Remove the dated docs (confirm the panic-audit disposition first)
7. Check no doc links to a removed file
8. (Optional) add `validate-docs.sh` so the drift cannot silently return
