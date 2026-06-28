# How Rigorix Was Built

**146,312 lines of Rust — 3 crates — 30 modules — 11 active days — 1 developer**

---

## The Numbers

| Metric | Value |
|--------|-------|
| **Timeline** | June 13 → June 28, 2026 (11 active days, 4-day gap Jun 22–26) |
| **Commits (all branches)** | 580 (52.7 commits/active day) |
| **Commits (main branch)** | 398 |
| **Contributors** | 1 (arman / Arman Wolkensteiner-Jalili) |
| **Rust source files** | 733 across all crates |
| **Lines of Rust code** | 146,312 (engine: bulk, cli + actions: remainder) |
| **Module count** | 30 (29 in `engine/src/`, plus CLI and Actions crates) |
| **Test annotations** | 2,295 (`#[test]` × 1,079 + `#[tokio::test]` × 1,216) |
| **Integration test files** | 30 (9 dedicated `tests/` + 21 `*_tests.rs` in src) |
| **CI verification steps** | 86 (6 stages, 193 total script files across all crates) |
| **Benchmarks** | 1 (criterion, DAG engine performance) |
| **Dependencies** | 69 across 3 crates (27 engine, 18 cli, 24 actions) |
| **Architecture documents** | 614 files across `.pi/` directories |
| **Module specs** | 38 (engine modules + CLI boundary + actions modules) |
| **ADRs** | 18 Architecture Decision Records |
| **Issue drafts** | 44 (across epics) |
| **Feature branches** | 160+ |


## Timeline: Active Days (11 of 16 calendar days)

```
Active   Date    Commits  What was built
────────────────────────────────────────────────────────────────────
  1    Jun 13      50    Engine scaffold, 29 module directories,
                          initial types (TaskGraph, planning pipeline)

  2    Jun 14      91    Core domain layer — 15+ module foundations:
                          execution_engine, event_system, cancellation,
                          audit, orchestration, TUI scaffold

  3    Jun 15      51    Batching — testing deep, architecture
                          improvements (M-07, M-08), tooling & CI,
                          testing expansion

  4    Jun 16     103    CLI boundary implementation: dispatcher,
     (peak)              config, signal handling, output formatting,
                          orchestrator wiring. CliParser (Clap).
                          Contract freeze for cli-boundary.

  5    Jun 17      35    TUI fixes — LLM token display, DAG node
                          counts, template persistence, orchestrator
                          spawning from TUI

  6    Jun 18      47    Tree-sitter anchored file_patch tool,
                          EventBridge node matching, config fixes,
                          runtime state cleanup

  7    Jun 19      78    Failure parser epic — TypeScript parser,
                          FixSuggestion service with symbol matching,
                          proofing scripts, architecture readiness

  8    Jun 20      50    Actions crate: action-output epic full
                          (5 components, 57 tests), policy-evaluator,
                          diff-analyzer, security-config

  9    Jun 21      34    Guardian framework init, clippy zero-gate
                          pass, gitnexus integration, READMEs,
                          LICENSE files, badges, comparison table
                         ─────────────────────────────────────
                           June 22–26: no commits (gap)
                         ─────────────────────────────────────
 10    Jun 27      23    Final polish: CI pipeline workflows,
                          demo video, contract-freeze refactoring,
                          dependency alignment, README finalization
                         ─────────────────────────────────────
                           June 22–26: no commits (gap)
                         ─────────────────────────────────────
 11    Jun 28      34    CI fix marathon: 37→0 failures across 86
                          steps — ((PASS++)) root cause in 47 scripts,
                          contract checker path drift, coverage checker
                          bugs, flaky env-var tests, architecture
                          readiness docs, local CI tooling, README
                          rewrite with templates + comparison + CI
```

**Build sprints by volume:**

| Phase | Active Days | Commits | Output |
|-------|------------|---------|--------|
| Engine foundation (modules 1–25) | 3 (Jun 13–15) | 192 | DAG, planning, execution, templates, observability, state, audit, batch improvements |
| CLI + TUI | 3 (Jun 16–18) | 185 | CLI boundary, ratatui-based TUI, flag-based scripting, tree-sitter tools |
| GitHub Actions + failure parser | 2 (Jun 19–20) | 128 | 10 action modules, failure parser, policy engine, diff-analyzer |
| CI, docs, hardening, polish | 3 (Jun 21 + 27 + 28) | 98 | Multi-stage workflows, docs, licenses, CI fix marathon, local CI tooling, README rewrite |


## Architecture at a Glance

```
┌──────────────────────────────────────────────────────────────┐
│                        rigorix-cli                           │
│          TUI (ratatui)  │  CLI (clap flag-based)            │
│          CLI Boundary (orchestrator, config, dispatch)      │
└────────────────────────────────┬─────────────────────────────┘
                                 │
┌────────────────────────────────▼─────────────────────────────┐
│                     rigorix-engine (core)                     │
│                                                              │
│  Planning:       planning → templates → dag_engine           │
│                   template_generation → plan_validation      │
│                                                              │
│  Execution:      execution_engine → tools → enforcement     │
│                   risk_gating → budget_tracking              │
│                                                              │
│  Governance:     policy_engine → quality_gates → permission │
│                   hooks → recovery_recipes                   │
│                                                              │
│  Infrastructure: event_system → state_persistence → audit   │
│                   observability → code_graph → repo_engine  │
│                                                              │
│  Cross-cutting:  configuration → cancellation → error       │
│                   failure_classification → failure_parser   │
│                   llm_step → common                          │
└────────────────────────────────┬─────────────────────────────┘
                                 │
┌────────────────────────────────▼─────────────────────────────┐
│                    rigorix-actions                            │
│  action-input → action-output → action-entrypoint           │
│  security-config → audit-posting → ci-integration           │
│  policy-evaluator → diff-analyzer                            │
└──────────────────────────────────────────────────────────────┘
```

**30 modules total — every one follows the same Clean Architecture layering:**

```
module/
├── domain/        # Entities, value objects, error enums
├── application/   # Service traits, DTOs, factories
├── infrastructure/# Repository implementations
├── interfaces/    # HTTP, event contracts
└── mod.rs         # Module root
```


## Methodology: What Made This Possible

### 1. Code Knowledge Graph

The entire codebase is indexed by **GitNexus** — a persistent Neo4j-compatible graph (KuzuDB) containing:

- **18,904 symbols** (functions, structs, traits, modules)
- **34,823 relationships** (calls, implements, extends)
- **300 execution flows** (end-to-end process traces)

Before editing any symbol, impact analysis answers: *"What breaks if I change this?"* — scoped to direct callers, full blast radius, and affected processes. This eliminated guesswork across 30 interconnected modules.

### 2. AI-Augmented Pipeline (Guardian)

Development followed a structured pipeline inside the **pi** coding agent:

```
For each issue:
  implement  →  validate  →  create PR  →  merge
```

The pipeline ran on **120+ feature branches**, each going through:

- **Contract freeze** — define module interfaces, traits, DTOs before implementation
- **Proofing** — add CI scripts, runbook, disaster recovery plan
- **Architecture readiness** — final alignment against Clean Architecture rules

Each step had automated acceptance gates: CI, tests, security scan, canonical doc sync, architecture conformance.

### 3. Multi-Stage CI

Three CI workflows enforce quality at different stages:

| Workflow | Trigger | Stages | Target Time |
|----------|---------|--------|-------------|
| **Preflight** | Every PR update | cargo check + clippy + fmt + quick tests | <3 min |
| **Full CI** | Push to feat/fix/main | lint → build → test → security → docs → coverage → integration → gate | ~10 min |
| **Hardening** | Push to main | 10 stages (docs → arch → lint → analysis → units → integration → security → migrations → build → readiness) | ~15 min |

All three workflows run on **cached Rust builds** via a shared setup action, using hash-based cargo registry and target caching.

### 4. Local CI Tooling

The same checks run locally through `bash .pi/scripts/local-ci.sh`, which discovers and executes all 86 verification steps across all three crates. A single command validates the full pipeline without pushing to GitHub:

```bash
# Full CI simulation (86 steps, ~2 min)
bash .pi/scripts/local-ci.sh

# Run one stage or crate
bash .pi/scripts/local-ci.sh --stage=lint       # format + clippy only
bash .pi/scripts/local-ci.sh --crate=engine     # engine only

# Faster iteration
bash .pi/scripts/local-ci.sh --quick            # skip release builds

# Save report for debugging
bash .pi/scripts/local-ci.sh --save
```

On failure, the report ends with a summary of exactly which steps failed and why. The `--list` flag enumerates all 193 discoverable CI script files across the workspace.

#### Proofing Scripts: Per-Epic Validation

Every epic had to pass its own **proofing scripts** before the corresponding feature branch could merge. These are not generic linters — each module has dedicated `check_*_contracts.sh` and `check_*_coverage.sh` scripts that validate:

- **Contract implementation** — every documented trait, struct, and DTO in the module spec has a concrete implementation in source
- **Coverage threshold** — the module meets minimum test counts (50 per module, enforced per-component)
- **Architecture readiness** — runbook, DR plan, canonical doc sync, observability integration

These proofing scripts are organized per crate:

```
engine/.pi/scripts/ci/check_dag-engine_contracts.sh
engine/.pi/scripts/ci/check_dag-engine_coverage.sh
engine/.pi/scripts/ci/stage_dag-engine_proofing.sh
actions/.pi/scripts/ci/check_action-entrypoint_contracts.sh
actions/.pi/scripts/ci/check_action-entrypoint_coverage.sh
... 30+ module-level proofing scripts
```

The proofing pipeline worked in practice as a **hardening gate**: every epic branch that passed proofing also passed CI on merge. When proofing revealed drift (e.g., renamed interfaces, relocated files), the fix was always to either update the contract checker or align the code — never to disable the check.

For future development, this structure scales linearly: new modules need only:
  1. Define the module spec in `.pi/architecture/modules/`
  2. Add check_contracts and check_coverage scripts in `.pi/scripts/ci/`
  3. Wire into the existing stage proofing template
  4. Run `bash .pi/scripts/local-ci.sh --save` to verify

The local CI runner discovers scripts via glob pattern (`*_proofing.sh`, `validate-*.sh`), so new scripts are picked up automatically — no pipeline configuration changes needed.

### 5. Validation Scripts

Seven self-contained bash validators enforce policy without external dependencies:

| Validator | File | What It Checks |
|-----------|------|----------------|
| CI | `.pi/scripts/validate-ci.sh` | Build, compilation readiness |
| Tests | `.pi/scripts/validate-tests.sh` | Test results |
| Security | `.pi/scripts/validate-security.sh` | Dependency audit, secret leakage |
| Operations | `.pi/scripts/validate-operations.sh` | Production readiness |
| Architecture | `.pi/scripts/validate-architecture.sh` | Clean Architecture layering |
| Canonical | `.pi/scripts/validate-canonical.sh` | Doc-to-code synchronization |
| Integration | `.pi/scripts/validate-integration.sh` | Cross-component integration |

### 6. Architectural Governance

Every architectural decision is documented in **ADR format** (8 decisions):

| ADR | Decision |
|-----|----------|
| ADR-001 | Clean Architecture with bounded contexts |
| ADR-002 | TOML template format for DAG definitions |
| ADR-003 | Async trait-based LLM provider abstraction |
| ADR-004 | Autonomy presets (Default, Advanced, Aggressive) |
| ADR-005 | Event bus with broadcast + drain persistence |
| ADR-006 | Atomic write-rename for state persistence |
| ADR-007 | Risk gating with Low/Medium/High classification |
| ADR-008 | RAII-style budget reservation for LLM calls |

A **gap ledger** tracks resolved and outstanding architecture gaps. The **canonical reference system** ensures docs don't drift: every generated doc references its source, and `validate-canonical.sh` enforces sync.

### 7. Merge Flow

```
feature branch → PR (gh CLI) → CI runs → squash merge → branch deleted
```

Every merge uses `gh pr merge --squash --delete-branch`, keeping `main` linear and clean. The MR validation script checks CI status, merge conflicts, architecture conformance, and test coverage before allowing merge.

### 8. Release Flow

```
git tag v*.*.* ⟶ cargo publish (engine → cli → actions) ⟶ GitHub Release
```

- Tag version validated against all crate versions
- Crates published in dependency order
- GitHub release auto-generated with changelog


## By the Numbers: Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Clean Architecture** | 30 modules with domain/application/infrastructure — each independently testable, swappable |
| **TOML templates** | Deterministic DAG definitions vs. free-form LLM planning — auditable, reviewable |
| **HMAC audit** | Every execution produces a signed envelope — verifiable trail, no tampering |
| **Risk gating** | Tool execution classified Low/Med/High — no uncontrolled shell access |
| **Policy engine** | Lane-based PR governance — `deny/review/flag` per scope |
| **Single developer** | AI tooling compensates for team size — code graph + pipeline + validators |
| **10 active days** | Granular issues (44 issue drafts) → parallel feature branches → fast CI turnaround; 54.6 commits/active day |


## Repository Snapshot

```
$ git rev-list --count HEAD
     398 (main branch)

$ git log --all --oneline | wc -l
     580 (all branches)

$ git shortlog -sn
   343  arman
   237  Arman Wolkensteiner-Jalili

$ find . -name '*.rs' -not -path './target/*' | wc -l
     733 (all Rust files including build scripts, examples, nested benches)

$ find engine/src -name 'mod.rs' | wc -l
     237 (237 clean-architecture modules with mod.rs entry points)

$ find . -name '*.md' -not -path './target/*' -not -path './.git/*' -not -path './node_modules/*' | wc -l
     ~800 documents

$ find .pi engine/.pi cli/.pi actions/.pi -name '*.md' 2>/dev/null | wc -l
     614 architecture documents

$ find .pi/architecture/modules engine/.pi/architecture/modules cli/.pi/architecture/modules actions/.pi/architecture/modules -name '*.md' -not -name '*template*' 2>/dev/null | wc -l
      38 module specs

$ find .pi/architecture/decisions engine/.pi/architecture/decisions cli/.pi/architecture/decisions actions/.pi/architecture/decisions -name '*.md' 2>/dev/null | wc -l
      18 ADRs

$ git log --all --format="%ai" | sort | head -1
  2026-06-13 07:32:12 +0200

$ git log --all --format="%ai" | sort | tail -1
  2026-06-28 14:44:49 +0200
```


## Artifacts Reference

| What | Where | Evidence |
|------|-------|----------|
| Source code (engine) | `engine/src/` — 524 .rs files | 29 modules, each with domain/application/infrastructure |
| Source code (cli) | `cli/src/` — 40 .rs files | CLI boundary + ratatui TUI |
| Source code (actions) | `actions/src/` — 159 .rs files | 10 action modules |
| CI pipeline | `.github/workflows/ci.yml` | 8 stages, parallel per crate |
| Preflight | `.github/workflows/preflight.yml` | <3 min, runs on every PR |
| Hardening | `.github/workflows/hardening.yml` | 10 stages, runs on main |
| Release | `.github/workflows/release.yml` | Tag-based crates.io + GitHub release |
| ADRs | `.pi/architecture/decisions/` | 8 decisions documented |
| Module specs | `.pi/architecture/modules/` | 42 module contracts |
| Validation scripts | `.pi/scripts/validate-*.sh` | 7 standalone validators |
| Merge script | `.pi/scripts/merge-mr.sh` | Squash merge via gh CLI |
| MR validation | `.pi/scripts/mr-validation.sh` | Pre-merge gate |
| Branch creation | `.pi/scripts/create-feature-branch.sh` | Creates branch + plan file |
| Code graph | `.gitnexus/meta.json` | 18,904 symbols, 34,823 relationships |
| Demo video | `rigorix-demo.mov` (16 MB) | Walkthrough of planning + execution |
| Architecture diagrams | `.pi/architecture/diagrams/` | System overview |
| Gap ledger | `.pi/architecture/gap-ledger.md` | Resolved/outstanding gaps |
| Architecture changelog | `.pi/architecture/CHANGELOG.md` | Change tracking |

---

*Built June 13–28, 2026 — 11 active days (4-day gap Jun 22–26) — 146,312 LOC — 30 modules — 2,295 tests — 580 commits across all branches — 1 developer — with assistance from AI-augmented tooling (GitNexus code graph, Guardian pipeline, pi agent harness).*
