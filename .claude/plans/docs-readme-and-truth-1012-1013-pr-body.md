# docs: make the README convert, and make every doc traceable to code (#1012, #1013)

Two defects, one root cause: the docs drifted from the code and nothing checked
them. **Every figure below was re-derived from the repository**, per #1012's
instruction not to reuse its own numbers.

> ## ⚠️ One acceptance criterion is deliberately NOT met
>
> **#1012 AC3 — "a visual (GIF/video) shows a refusal scenario"** is deferred. I
> have no way to produce a genuine screen recording, and this machine has no font
> rasterizer (no PIL, no ImageMagick; ffmpeg here has no drawtext/freetype). A
> synthetic terminal animation would be a fabrication dressed as a capture.
>
> `docs/assets/README.md` documents the required asset and how to record it, and
> the README carries a comment marking the spot. The README intentionally does
> **not** reference an image that does not exist — a broken `<img>` is worse than
> none. **The user is recording this.** AC3 stays open until the file lands.

## #1012 — the README as a product page

- **Headline leads with the problem**, not the technology, with the one-line value
  proposition and the buyer named.
- **Badges** — `rigorix-verifier` on crates.io / npm / PyPI (all verified published
  at 0.5.0) plus CI and licence. The single hand-written `rigorix-cli` badge was
  not an ecosystem signal; the verifier packages are.
- **New section: the v1.10.0 capability, in both registers.** Technical —
  preconditions evaluated outside the agent-writable workspace, a refusal recorded
  in a signed envelope, verification by **re-derivation** (spec §4.3.1), NC-4.
  Business — an auditor proving what an agent was permitted to do without trusting
  the vendor. Both bounded by the L1/L2/L3 claim ceilings.

## #1013 — the comparison table was a category error

Deleted the Claude Code / Copilot / Cursor / Aider / SWE-Agent columns: those are
the agents Rigorix *governs*, and the README says so in its own words.

Rebuilt on the real neighbours, **every cell read from the vendor's own site or
repository on 2026-10-15** — and there is network access, so this was re-verified
rather than copied: Cordum (`cordum.io`), Mault (`mault.ai`), APERION
(`aperion.ai`, `gh api repos/AperionAI/shield`), microsoft/agent-governance-toolkit
(6,430 stars, MIT, 10/10 OWASP Agentic Top 10). Unestablishable cells read **"not
established"** rather than inferring a weakness. Added the **"what this table is
not"** note: not a benchmark, no independent comparative evaluation.

## #1013 — release notes have a durable home

`CHANGELOG.md` (Keep a Changelog, newest first), built from the **merged PRs and
the issues closed in each version's window** — 14 releases backfilled — so entries
and tags cannot disagree.

**Fixed a real defect the issues missed:** `release.yml` told every reader to "See
[CHANGELOG](.pi/architecture/CHANGELOG.md)". That file is the **Guardian
framework's generated architecture log** (`DO NOT EDIT GENERATED FILES`, mirrored
into 5 trees), not Rigorix's release history. Every release pointed readers at the
wrong document.

Process documented in `CONTRIBUTING.md` so the next release follows it.

## Every stale figure fixed (re-measured, not copied)

| claim | was | now | source |
|---|---|---|---|
| OSS tools | 14 | **15** (+3 auth tools the README omitted) | `all_tool_descriptors()` |
| CI steps | "113 automated verification steps" — **not derivable** | 10 `ci.yml` jobs; 114 local steps from 180 scripts, + the commands to re-derive | `local-ci.sh --list` |
| engine modules | 30 | **35** (32 bounded contexts + 3 cross-cutting) | `engine/src` |
| mcp modules | 6 | **8** | `mcp/src` |
| mcp audit tools | 3 | **4** | `mcp/src/host/mod.rs` |
| cli audit subcommands | `list\|show\|diff` | + `projection`, `verify-projection` | `rigorix-cli audit --help` |

## Two corrections to the issues' own research

Both would have **introduced** errors:

1. `actions/README.md`'s `version = "1.0.0"` is a **policy document version**
   (`policy_evaluator/domain/types.rs`: "The policy document version string"), not
   the crate version. **Left alone.**
2. `actions/src` has **9** bounded contexts, not 10 — the issue counted `src`
   itself. The README's "9" was already correct.

## The private-repo problem (thank you for flagging it)

`rigorix-sdk` is **not public**, so this public repo was linking readers to a repo
they cannot open. Removed every such link, including the same broken URL in
`server/src/event_bridge.rs`. The polyglot-verifier claim now rests on the three
published registries, which anyone can check.

## The durable fix (#1012's own suggestion)

**`.pi/scripts/validate-docs.sh`**, registered **gating** in `.pi/guards.tsv`. It
fails when a relative doc link does not resolve, or when `CHANGELOG.md`'s newest
version disagrees with `engine/Cargo.toml`. `--self-test` proves it bites (aligned
passes / broken link bites / changelog mismatch bites).

It immediately found **four real pre-existing defects**, all fixed here:

- 3 dangling links — `check_action-output_coverage.sh`,
  `code_graph/interfaces/http/mod.rs` ×2
- **`engine/docs/runbook-code-graph.md` documented `GET /api/v1/code-graph/health`
  and a `HealthResponse` schema "from HTTP API contracts".** `code_graph` has no
  `interfaces/` layer, no such type and no such route. Per #1012's rule the claim
  was **deleted, not implemented**.

## Removals — blast radius checked, not assumed

Removed `next-workstreams-2026-09-21.md`, `contract-conformance-2026-09-08.md`,
`sdk-server-strategy-freeze-2026-09-06.md`. The issues said "0 refs", which is true
for **code/scripts** — but **two were cited by the canonical ADR-016**, so its
dangling references were rewritten in the same change (AC10 requires no doc to
point at a deleted file).

`panic-audit-2026-10-12.md` is **kept** — the panic baseline and
`docs/security-guards.md` cite it. `docs/issues/**` is **kept** — historical work
records, explicitly not to be deleted as a side effect.

## Acceptance criteria

| # | #1012 | Status |
|---|---|---|
| 1 | Headline leads with the problem | ✅ |
| 2 | One line states who and why | ✅ |
| 3 | **A visual shows a refusal** | ⚠️ **DEFERRED — user recording** |
| 4 | Badges: crates/npm/PyPI verifiers, build, licence | ✅ |
| 5 | Every README figure derivable and current | ✅ |
| 6 | v1.10.0 capability, technical + business | ✅ |
| 7 | Per-crate READMEs carry no stale version/count | ✅ (2 issue claims were themselves wrong) |
| 8 | No doc asserts an unimplemented capability | ✅ (found + deleted a real one) |
| 9 | Dated docs removed; survivors justified | ✅ |
| 10 | No doc points at a deleted file | ✅ enforced by the new guard |

| # | #1013 | Status |
|---|---|---|
| 1 | No coding assistant listed as a competitor | ✅ |
| 2 | Every competitor cell sourced + dated | ✅ (re-verified live) |
| 3 | Every Rigorix advantage traceable | ✅ |
| 4 | Release notes in-repo, not in README | ✅ |
| 5 | Changelog format chosen and documented | ✅ |
| 6 | Table states what it does not establish | ✅ |

## Validation

```
bash .pi/scripts/validate-docs.sh              # + --self-test
bash .pi/scripts/validate-guards.sh            # all gating guards
bash .pi/scripts/validate-canonical.sh
bash .pi/scripts/validate-architecture.sh
cargo check -p rigorix-server
```

Refs #1012 · #1013
