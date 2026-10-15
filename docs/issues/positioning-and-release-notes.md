---
guardian_issue:
  id: "ISSUE-POSITIONING-AND-RELEASE-NOTES"
  title: "Competitive positioning: replace the category-error comparison table, and give release notes a durable home"
  epic: "adoption / documentation"
  component: "Docs"
  module: "documentation"
  status: planned
  priority: high
  dependencies:
    - "ISSUE-DOCS-PRODUCT-PAGE-AND-CODE-TRUTH (#1012) — the same README rewrite; land these together"

  intent: |
    Two product-communication defects.

    1. The README's comparison table is a CATEGORY ERROR. It compares Rigorix to
       Claude Code, Copilot/Cursor, Aider and SWE-Agent — which are the coding
       agents Rigorix GOVERNS, not products it competes with. It reads as though
       Rigorix is an alternative to an agent, which is the opposite of the
       positioning ("a governance layer underneath yours"). It must be replaced
       with the actual comparables, on dimensions that can be verified.

    2. Release notes have no durable home. There is no CHANGELOG.md; release
       history exists only as generated GitHub Releases. It must live in the repo,
       and it must not be in the main README.

  in_scope:
    - "Replace the comparison table with verified comparables (list below)"
    - "State the comparison date and the evidence source for every competitor cell"
    - "Decide and create the release-notes home (CHANGELOG.md vs docs/releases/)"
    - "Keep release history out of the main README"
    - "Consider having the release workflow append to the changelog it already generates"

  out_of_scope:
    - "The README rewrite itself (headline/value prop/visual/badges) — that is #1012"
    - "Making Rigorix better on any dimension where the comparison is unfavourable.
       If a competitor wins a row, the row says so or is removed; it is not spun."
    - "Claims about competitors that cannot be sourced → leave the cell as
       'not established' rather than guessing"

  affected_layers:
    domain:
      - "No code change"
    application:
      - "No code change"
    infrastructure:
      - "Possibly: .github/workflows/release.yml (append generated notes to the changelog)"
    docs:
      - "Modified: README.md (the comparison table)"
      - "Created: CHANGELOG.md or docs/releases/ (decision)"
      - "Possibly: docs/README.md as a documentation index"

  canonical_references:
    - module: "docs/spec/README.md (claim ceilings bound what may be claimed)"
    - module: "rigorix-sdk (the polyglot verifiers — the strongest verifiable differentiator)"
    - module: ".github/workflows/release.yml (generates the current release notes)"
    - last_sync: "2026-10-14"

  acceptance_criteria:
    - "No coding assistant appears in the comparison table as a competitor"
    - "Every competitor row is sourced and dated; unsourced cells read 'not established'"
    - "Every Rigorix advantage in the table is traceable to code or a published spec"
    - "Release notes live in-repo, versioned, and are NOT in the main README"
    - "The changelog format is chosen and documented so the next release follows it"
    - "The comparison states what it does not establish (e.g. that it is not a benchmark)"

  validators:
    - canonical
    - docs

  implementation_notes: |
    ## Defect 1 — the table compares Rigorix to the things it governs

    Current table: Rigorix vs Claude Code vs Copilot/Cursor vs Aider vs SWE-Agent.

    Those are agent products. Rigorix is a governance layer *underneath* agents —
    the README says so elsewhere ("Rigorix isn't a separate agent — it's a
    governance layer underneath yours"). Putting them in one table contradicts the
    product's own thesis and invites the reader to conclude Rigorix is a worse
    agent rather than a different layer.

    ## The actual comparables (researched 2026-10-14)

    Neighbours — same problem, same category:

    | product | positioning (their words) | notes |
    |---|---|---|
    | **Cordum** (`cordum.io`) | "The Action Firewall for AI Agents" | "policy, approval, and redacted audit evidence in front of Claude Code, MCP tools, shell commands, workflows, and production changes"; pre-tool gate with a REQUIRE_APPROVAL decision and an action hash a reviewer approves; source-available core; publishes a "CAP v2 protocol spec" |
    | **Mault** (`mault.ai`) | "Governance for Coding Agents" | "applies your rules deterministically at every step from keystroke to merge, with a signed record of every action"; "No LLM in the enforcement path"; local; compliance badges (SOC 2, ISO 27001, EU AI Act, FINOS) |
    | **APERION** (`aperion.ai`, repo `AperionAI/shield`) | "Runtime AI Governance for Enterprise Agents" | on-premises; "stops the action that exceeds the agent's authority, and hands the examiner a record bound to the human who owns it"; the `shield` repo is a free local MCP guardrail with 65+ rules, native PreToolUse hooks, and a block demo GIF |
    | **microsoft/agent-governance-toolkit** | "Policy enforcement, zero-trust identity, execution sandboxing" | MIT, ~6.4k stars, claims 10/10 OWASP Agentic Top 10 coverage — the most credible open-source neighbour |
    | **AGCP.ai**, **LangGuard**, **ControlTower OS** | "deterministic runtime AI governance" / "AI execution control" | same vocabulary; verify individually before citing |

    Adjacent, not competitors:

    - **Microsoft Agent Hooks** — an open, framework-neutral governance *contract*
      (a standard). Rigorix could be *compatible* with it; listing it as a rival
      would be a mistake.
    - **OPA / Styra** — policy-as-code for static request/response authorization.
      Not agent execution; relevant only as a contrast.
    - **agentsh and OS-level sandboxes** — block syscalls, not semantics.

    Wrong category — must not appear:

    - **Credo AI** — AI governance/compliance platform (policy registers, model
      risk). Not execution enforcement.
    - **Opaque** — confidential computing. Unrelated.
    - **SICKR.ai** — workforce governance across humans and agents. Broader topic.

    ## Where Rigorix is verifiably different (traceable to code)

    Only these, and each must be backed by an artifact:

    1. **Open-source polyglot verifiers.** `rigorix-verifier` on crates.io, npm and
       PyPI; a Go verifier and Java client in `rigorix-sdk`; a GitHub Action. A
       neighbouring product hands you *its* record; Rigorix hands you the verifier
       you can run yourself, in your own language. **This is the strongest and most
       checkable differentiator.**
    2. **Verification by re-derivation, not by trusting a signature.** The bundle
       is checked by re-deriving from the anchored chain (spec 4.3.1) — a signature
       proves *who said it*, not *whether it is so*.
    3. **Moment separation.** Plan-time, approval-time and dispatch-time
       re-validation, so a plan that was permitted earlier is re-checked against
       authority as it exists now (ADR-017).
    4. **Cross-run, identity-bound records.** HMAC envelopes plus an anchor
       (ADR-016), so the trail survives across runs rather than describing one
       session.
    5. **Canonical effect keys** — value-identity resolution so the same economic
       action is recognised across different identifiers.
    6. **A stated claim ceiling.** The artifact never asserts present standing
       (NC-4). Being explicit about what is *not* claimed is itself a differentiator
       in a field with a lot of overclaim.

    ## Rules for the table (this is the part that keeps it honest)

    - **Source and date every competitor cell.** Competitors ship weekly; a table
      without a date is wrong within a month. Cite the page or repo and the date it
      was read.
    - **Unverifiable cell → "not established".** Do not infer a competitor's
      weakness from absence of evidence. "We could not establish X" is acceptable;
      "they lack X" without a source is not.
    - **Do not claim a dimension Rigorix also does not have.** If Rigorix cannot
      demonstrate the row, the row goes.
    - **State what the table is not** — it is not a benchmark, and no independent
      comparative evaluation has been performed (consistent with the L1/L2/L3 claim
      ceilings). Optionally, invite the comparison to be falsified.

    ## Defect 2 — release notes have no home

    Measured 2026-10-14: **no `CHANGELOG.md` exists.** The only release history is
    the generated GitHub Releases (`v1.10.0`, `v1.9.5`, `v1.9.4`, `v1.9.3`, …),
    produced by `.github/workflows/release.yml` in a "Generate changelog" step.

    Decide ONE home and document it:

    - `CHANGELOG.md` at the repo root (Keep a Changelog conventions, newest first,
      one entry per released version, linked to the GitHub Release) — **recommended**;
      or
    - `docs/releases/vX.Y.Z.md` if entries need length.

    Two constraints:

    - it must be **in the repo**, so the history is present at a clone without a
      network round-trip;
    - it must **not** be in the main README. The README is a product page; a
      version list belongs in the changelog, with the README linking to it.

    The release workflow already generates per-release notes — consider having it
    also prepend that content to the changelog, so the changelog cannot drift from
    the tags. Note `1.9.x` historical entries would need backfilling from the
    existing GitHub Releases.

  file_changes:
    - "modify: README.md (replace the comparison table; link to the changelog)"
    - "create: CHANGELOG.md (or docs/releases/)"
    - "modify (optional): .github/workflows/release.yml (append generated notes)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-POSITIONING-AND-RELEASE-NOTES

## Defect 1 — the comparison table is a category error

The README compares Rigorix against **Claude Code, Copilot/Cursor, Aider, SWE-Agent**.
Those are the agents Rigorix governs. The README says so in its own words:

> "Rigorix isn't a separate agent — it's a governance layer underneath yours."

Putting them side by side contradicts the thesis and makes Rigorix read like a
worse agent instead of a different layer.

## The real comparables (researched 2026-10-14)

**Same category:**

| product | their positioning | why it is relevant |
|---|---|---|
| **Cordum** | "The Action Firewall for AI Agents" | policy + approval + redacted audit evidence *before* execution, in front of Claude Code / MCP / shell; action-hash approval; source-available core |
| **Mault** | "Governance for Coding Agents" | deterministic rules keystroke→merge, **signed record of every action**, explicitly "no LLM in the enforcement path", local |
| **APERION** | "Runtime AI Governance for Enterprise Agents" | on-prem; stops actions exceeding authority; record bound to a human owner; `AperionAI/shield` is a free local guardrail with PreToolUse hooks and a demo GIF |
| **microsoft/agent-governance-toolkit** | policy enforcement + zero-trust identity + sandboxing | MIT, ~6.4k stars, 10/10 OWASP Agentic Top 10 — the most credible open-source neighbour |
| AGCP.ai · LangGuard · ControlTower OS | "deterministic runtime AI governance" | same vocabulary; verify individually first |

**Adjacent, not rivals:** Microsoft **Agent Hooks** (an open governance *contract* —
Rigorix should aim to be compatible, not competitive), **OPA/Styra** (static
request authorization), **agentsh / OS sandboxes** (syscall-level, no semantics).

**Wrong category, must not appear:** **Credo AI** (AI governance/compliance
platform), **Opaque** (confidential computing), **SICKR.ai** (workforce governance).

## Where Rigorix is verifiably better

Each of these is traceable to a shipped artifact — no others may be claimed:

| # | differentiator | artifact |
|---|---|---|
| 1 | **Open-source polyglot verifiers** — competitors hand you their record; Rigorix hands you the verifier | `rigorix-verifier` on crates.io / npm / PyPI; Go + Java in `rigorix-sdk`; GitHub Action |
| 2 | **Verification by re-derivation, not by signature** | spec §4.3.1 |
| 3 | **Moment separation** (plan / approval / dispatch) | ADR-017 |
| 4 | **Cross-run identity-bound records** with an anchor | ADR-016 |
| 5 | **Canonical effect keys** (value-identity resolution) | `eff:` keys in the envelope |
| 6 | **A stated claim ceiling** — never asserts present standing | spec, NC-4 |

## The rules that keep the table honest

1. **Source and date every competitor cell.** A table without a date is wrong
   within a month.
2. **Unverifiable cell → "not established."** No inferred weaknesses. "We could not
   establish X" is fine; "they lack X" without a source is not.
3. **Never claim a row Rigorix cannot itself demonstrate.**
4. **State what the table is not.** It is not a benchmark; no independent
   comparative evaluation exists (L1/L2/L3 claim ceilings).

## Defect 2 — release notes need a home

```
CHANGELOG.md         ABSENT
release history      GitHub Releases only (v1.10.0, v1.9.5, v1.9.4, v1.9.3, …)
who generates it     .github/workflows/release.yml -> "Generate changelog"
```

Pick **`CHANGELOG.md`** at the root (Keep a Changelog, newest first, linked to the
GitHub Release) or `docs/releases/`. It must be in-repo, and it must **not** be in
the main README — the README links to it. Backfill `1.9.x` from the existing
Releases, and consider having the release workflow append automatically so the two
cannot drift.

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | No coding assistant listed as a competitor | review |
| 2 | Every competitor cell sourced + dated; unsourced reads "not established" | review |
| 3 | Every Rigorix advantage traceable to code or spec | `validate-canonical.sh` |
| 4 | Release notes live in-repo and are not in the README | `validate-docs.sh` |
| 5 | Changelog format chosen and documented | review |
| 6 | The table states what it does not establish | review |

## Implementation

> **Agent:** Re-verify every competitor claim at implementation time — this research
> is dated 2026-10-14 and competitors ship weekly. If you cannot source a cell,
> write "not established". Do not spin an unfavourable row; remove it.

### Steps

1. Re-read each competitor's site/repo; capture the exact wording and the date
2. Rebuild the table on the six verifiable dimensions; mark unknowns explicitly
3. Delete the coding-assistant columns entirely
4. Choose the changelog home; create it; backfill from the GitHub Releases
5. Link the README to the changelog; remove any version list from the README
6. (Optional) wire the release workflow to append
7. Add the "what this table is not" note
