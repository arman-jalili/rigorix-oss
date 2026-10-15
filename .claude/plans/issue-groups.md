# Issue Groups — documentation / positioning

Fetched 2026-10-15. **2 open issues.** Both filed by `e57459d3` / `7b35bf60` (docs issues).

| # | Title | Component | Priority |
|---|-------|-----------|----------|
| #1012 | Documentation hygiene: the README must convert, and every doc must be verified against code | Docs | high |
| #1013 | Competitive positioning: replace the category-error comparison table, and give release notes a durable home | Docs | high |

## Grouping

| Group | Type | Issues | Branch |
|-------|------|--------|--------|
| **D** | Related batch (same files: `README.md`) | #1012 + #1013 | `docs/readme-and-truth-1012-1013` |

#1013 declares #1012 as its dependency: "the same README rewrite; land these
together". Both rewrite `README.md`, so separate PRs would conflict.

## Re-measured figures (the issues forbid reusing theirs)

| figure | issue says | **measured 2026-10-15** | source |
|---|---|---|---|
| OSS tool descriptors | 15 | **15** ✅ | `all_tool_descriptors()` |
| auth tools the README omits | 3 | **3** ✅ | `mcp/src/auth/interfaces/mcp/mod.rs` |
| CI jobs in `ci.yml` | 10 | **10** ✅ (workspace, lint, build, test, coverage, audit, docs, integration, conformance, gate) | `.github/workflows/ci.yml` |
| "113 automated verification steps" | not derivable | **not derivable** — local-ci discovers 25 root + 87 engine + 28 cli + 40 actions = 180 scripts | `.pi/scripts/local-ci.sh --list` |
| `mcp/src` module dirs | 9 | **9** (incl. `mcp/src`) → **8** bounded contexts | `find` |
| `actions/src` module dirs | 10 | **10** (incl. `src`) → **9** bounded contexts | `find` |
| coverage gate | ≥60% ✅ | **≥60%** ✅ | `.pi/scripts/coverage.sh` |
| actions version | 1.4.0 | **1.4.0** ✅ | `actions/Cargo.toml` |
| root `CHANGELOG.md` | absent | **absent** ✅ | `ls` |

## Defect the issues missed

`.github/workflows/release.yml:133` emits, into every GitHub Release:

```
See [CHANGELOG](.pi/architecture/CHANGELOG.md) for details.
```

That path is the **Guardian framework's generated architecture change log**
(`Blueprint Source: Guardian Framework v1.2`, `DO NOT EDIT GENERATED FILES`),
mirrored into **5** trees — not Rigorix's release history. Every release points
readers at the wrong document. This is the sharpest justification for #1013's
"release notes need a durable home".

## Competitor research — re-verified live, 2026-10-15

Network access **is** available (`curl` + `gh api`); there is no browser tool, but
none is needed. Every cell below was fetched from the source today.

| product | verified positioning (verbatim) | source |
|---|---|---|
| **Cordum** | "The Action Firewall for AI Agents"; "policy, approval, and redacted audit evidence in front of Claude Code, MCP tools, shell commands…"; "Source-available core"; pre-tool gate w/ `REQUIRE_APPROVAL`; "An authorized reviewer can approve or reject this exact action hash."; "CAP v2 protocol spec" | cordum.io |
| **Mault** | "Governance for Coding Agents"; "applies your rules deterministically at every step from keystroke to merge, with a signed record of every action"; "**No LLM in the enforcement path**"; "Same input, same decision, every time"; "Signed, verifiable offline"; badges SOC 2 / GDPR / PCI / ISO 27001 / EU AI Act / FINOS | mault.ai |
| **APERION** | "Runtime AI Governance for Enterprise Agents"; "ON-PREMISES · RUNTIME GOVERNANCE"; "reads the call, stops the action that exceeds the agent's authority, and hands the examiner a record bound to the human who owns it" | aperion.ai |
| **AperionAI/shield** | local MCP guardrail; **11 stars**; rules counted in README: **68**; ⚠️ **licence CONFLICTS** — GitHub description says "Free, open-source", README badge says **"License: Proprietary"** | `gh api repos/AperionAI/shield` |
| **microsoft/agent-governance-toolkit** | "Policy enforcement, zero-trust identity, execution sandboxing…"; **6,430 stars**; **MIT**; "Covers 10/10 OWASP Agentic Top 10"; pushed 2026-10-15 | `gh api` |

### Corrections to the issue's own research

- **AperionAI/shield licence is not "free, open-source"** — the GitHub description
  says so, the README badge says **Proprietary**. The cell reads *conflicting*, with
  both sources named. Neither claim is asserted.
- **"65+ rules"** is not what the README says; it says **68** (other tables say 40
  and 270 patterns). Cite 68 with the source.
- Mault's "No LLM in the enforcement path" **is** verified (the issue's quote was
  close but not exact — it also says "no LLM in the path").

### Our differentiators — each traced to a shipped artifact

| # | differentiator | artifact (verified 2026-10-15) |
|---|---|---|
| 1 | Open-source polyglot verifiers | `rigorix-verifier` **0.5.0** on **crates.io** (1,302 downloads), **npm** (0.5.0), **PyPI** (0.5.0); Go + Java in `rigorix-sdk` |
| 2 | Verification by re-derivation, not signature | spec §4.3.1 |
| 3 | Moment separation (plan / approval / dispatch) | ADR-017 |
| 4 | Cross-run identity-bound records + anchor | ADR-016 |
| 5 | Canonical effect keys | `eff:` keys |
| 6 | A stated claim ceiling (never asserts present standing) | spec, NC-4 |

## Blast radius of the proposed deletions (measured, and it contradicts the issue)

#1012 lists 3 docs for removal with "0 refs". Measured refs **from code/scripts**
(0 ✅) **but refs from docs are not 0**:

| doc | code refs | doc refs | who cites it |
|---|---|---|---|
| `next-workstreams-2026-09-21.md` | 0 | 3 | **`engine/.pi/architecture/decisions/ADR-016`** (canonical) |
| `contract-conformance-2026-09-08.md` | 0 | 1 | a plan artifact |
| `sdk-server-strategy-freeze-2026-09-06.md` | 0 | 4 | **ADR-016** (canonical) |
| `panic-audit-2026-10-12.md` | 0 | 4 | `docs/security-guards.md` (must stay) + 3 `docs/issues/*` |
| `runbook.md` | 32 | 107 | keep |
| `dr-plan.md` | 10 | 43 | keep |
| `security-guards.md` | 1 | 15 | keep |

**Two of the removals are cited by an accepted, canonical ADR** (they are the
provenance for its backlog/strategy references). So AC10 ("no doc points at a
deleted file") and AC9 collide with the ADR unless the ADR is edited.

**Decision** (recorded, not silent): *keep* both ADR-cited documents and justify
them as decision provenance; *remove* `contract-conformance-2026-09-08.md` (no
canonical citation). For `panic-audit-2026-10-12.md`, fold its durable content into
`docs/security-guards.md` (which already references it) and keep the report, since
the guard baseline cites it. `docs/issues/*` is **kept** — history, not product
docs, and out of scope by the issue's own instruction.

## Validation per issue

| Issue | Validators |
|-------|-----------|
| #1012 | canonical, docs(→new `validate-docs.sh`), ci |
| #1013 | canonical, docs |
