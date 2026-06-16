# Guardian Agent Framework

**Version:** 1.2.0
**Status:** Template
**Architecture:** Pi-first

---

## Overview

This framework uses `.pi/` as the source of truth. Other formats (`.claude/`, `.opencode/`, `.agents/`, `.omp/`) are generated exports.

---

## Canonical Reference Requirement

**All implementation files must reference architecture documentation:**

```typescript
/**
 * Canonical Reference: .pi/architecture/modules/[module-name].md#[section]
 * Implements: [spec/AC from architecture]
 * Last Sync: [date from CHANGELOG]
 */
```

**Generated files must include source reference:**

```markdown
<!--
Canonical Reference: .pi/[source-path].md
Generated: [timestamp]
DO NOT EDIT DIRECTLY - Modify source in .pi/
-->
```

**Architecture changes require CHANGELOG entry:**

```markdown
## [date] - [Change Title]

### Changed
- Module: [name]
  - [what changed]

### Impact
- Files affected: [list]
- Canonical refs to update: [list]
- Validators: [which to re-run]

### Migration
[steps to update implementation]
```

**Validation:** `validate-canonical.sh` checks reference integrity, coverage, and architecture sync status.

---

## Directory Structure

```
.pi/
├── agent/
│   └── AGENTS.md              # Project instructions + workflow config (YAML front matter)
│
├── architecture/              # Architecture documentation (NEW)
│   ├── modules/               # Module architecture docs
│   │   ├── auth-system.md
│   │   ├── data-layer.md
│   │   ├── api-gateway.md
│   │   └── [module-name].md
│   ├── diagrams/              # Architecture diagrams
│   │   ├── system-overview.md
│   │   └── data-flow.md
│   ├── CHANGELOG.md           # Architecture change log (required)
│   └── decisions/             # Architecture Decision Records (ADR)
│       ├── ADR-template.md
│       ├── ADR-001-auth-strategy.md
│       └── ADR-002-database-choice.md
│
├── context/
│   ├── project.md             # Project facts, commands (template)
│   ├── patterns.md            # Code templates (template)
│   ├── checklists.md          # Validation checklists
│   └── output-formats.md      # Report templates
│
├── skills/
│   ├── agents/
│   │   ├── architecture-coordinator.md
│   │   ├── architecture-validator.md
│   │   ├── security-validator.md
│   │   ├── operations-validator.md
│   │   ├── test-validator.md
│   │   ├── integration-validator.md
│   │   ├── ci-mr-validator.md
│   │   ├── code-developer.md
│   │   ├── documentation-maintainer.md
│   │   ├── issue-creator.md
│   │   ├── commit.md            # Clean, logical commits
│   │   ├── push.md              # Keep remote branch current
│   │   ├── pull.md              # Sync with latest main
│   │   ├── land.md              # PR merge loop with validation
│   │   ├── debug.md             # Systematic debugging
│   │   ├── subagent-registry.md # Delegated subagent system with tool scoping
│   │   ├── plan-mode.md         # Queued edits for batch review
│   │   ├── snippets.md          # Reusable #handle prompt fragments
│   │   └── session-persistence.md # Session lifecycle management
│   └── validators/
│       ├── architecture-validator.md
│       ├── security-validator.md
│       ├── security-guards.md     # Path safety + command deny-list
│       ├── context-compaction.md  # Token-aware context management
│       ├── system-prompt-tiers.md # Full/lite prompt tiers by model
│       ├── model-registry.md      # Model capability scoring
│       ├── operations-validator.md
│       ├── integration-validator.md
│       ├── test-validator.md
│       └── ci-validator.md
│
├── prompts/
│   ├── feature-development.md
│   ├── bug-fix.md
│   ├── hotfix.md
│   ├── refactoring.md
│   ├── issue-implementation-series.md
│   ├── epic-plan.md           # Multi-module epic planning (overview + module slice)
│   ├── issue-draft.md         # Create draft issues from epic
│   ├── git-issues.md          # Create epics/issues in GitHub/GitLab
│   ├── issue-closeout.md      # Validate + create compliance MR
│   ├── issue-merge.md         # Merge MR + close issue + update tracking
│   ├── plan-to-issues.md      # Convert superpowers plan to issues
│   ├── blueprint-validate.md  # Validate blueprint integrity
│   ├── sync-check.md          # Check exports in sync with blueprint
│   ├── context-refresh.md     # Update context from codebase state
│   ├── scope-analyzer.md      # Auto-determine scope classification
│   ├── pattern-extract.md     # Extract patterns to blueprint
│   └── blueprint-update.md    # Reverse-sync implementation to blueprint
│
├── validators/                 # TOML-based declarative validation rules
│   ├── default.toml           # Built-in validators with inline tests
│   └── README.md              # Validator documentation
├── scripts/
│   ├── validate-ci.sh
│   ├── validate-tests.sh
│   ├── validate-operations.sh
│   ├── validate-security.sh
│   ├── validate-architecture.sh
│   ├── validate-canonical.sh  # Canonical reference integrity
│   ├── validation-cache.sh
│   ├── fetch-issues.sh
│   ├── categorize-issues.sh
│   ├── create-feature-branch.sh
│   ├── create-mr.sh
│   ├── mr-validation.sh
│   └── merge-mr.sh
│
├── extensions/
│   ├── validation-runner.ts   # Pi extension for validation commands
│   ├── coordinator.ts         # Pi extension for scope classification + validation tools
│   ├── bash-guard.ts          # Destructive command blocking + path safety guards
│   ├── filechanges.ts         # File change tracking with accept/decline
│   ├── read-only-mode.ts      # Safe exploration mode (read/grep/find/ls only)
│   ├── ask-user-question.ts   # Structured question tool (text/single/multi-select)
│   ├── config-reload.ts       # Dynamic config reload on AGENTS.md change
│   ├── plan-mode.ts           # Queued mutations for batch review (/plan)
│   ├── slash-commands.ts      # /init, /validate, /scope, /snippet commands
│   ├── session-persistence.ts # Structured session lifecycle with auto-titling
│   ├── snippets.ts            # #handle token expansion and management
│   └── redaction.ts           # Automatic secret redaction in output
│
├── workpad.md                 # Persistent session progress tracker
├── github/                    # GitHub Copilot CLI templates
│   ├── copilot-instructions.md    # Main project instructions
│   ├── instructions/
│   │   ├── architecture.instructions.md
│   │   └── validation.instructions.md
│   ├── agents/
│   │   ├── architecture-coordinator.agent.md
│   │   └── epic-planner.agent.md
│   └── copilot/
│       └── settings.json      # Copilot CLI settings
│
├── INDEX.md                   # This file
└── README.md                  # Complete documentation
```

---

## Architecture Module Documentation

Each module in `.pi/architecture/modules/` follows this structure:

```markdown
# [Module Name] Architecture

<!--
Canonical Reference: .pi/architecture/modules/[module-name].md
Source: Blueprint (do not modify implementation directly)
-->

## Overview
[Module purpose and scope]

## Components
| Component | File | Purpose |
|-----------|------|---------|
| [name] | src/[path] | [what it does] |

## Data Flow
[How data moves through this module]

## Dependencies
- Depends on: [other modules]
- Used by: [other modules]

## Security Considerations
[Security requirements for this module]

## Testing Requirements
[Unit, integration, e2e tests needed]

## Change Log References
- 2026-04-26: [change] → see CHANGELOG.md#[section]
```

---

## Repository Tool

The framework supports both GitHub and GitLab:

| Tool | CLI | Platform |
|------|-----|----------|
| `gh` | GitHub CLI | GitHub.com |
| `glab` | GitLab CLI | GitLab.com or self-hosted |

Selected during `guardian init` and used in all git-related workflows.

---

## Agent Directory

| Agent | Role | When to Use | Mode |
|-------|------|-------------|------|
| `architecture-coordinator` | Master orchestrator | All tasks | primary |
| `architecture-validator` | Architecture check | Moderate+ scope | subagent |
| `security-validator` | Security check | Complex+ scope | subagent |
| `operations-validator` | Operations check | Plan review | subagent |
| `test-validator` | Test validation | Post-code | subagent |
| `integration-validator` | Integration check | Complex+ scope | subagent |
| `ci-validator` | CI/merge | All PRs (automated) | subagent |
| `code-developer` | Implementation | All code tasks | subagent |
| `issue-creator` | Issue tracking | All tasks | subagent |
| `documentation-maintainer` | Doc sync | Architecture changes | subagent |

---

## Scope Classification

| Scope | Files | Lines | Required Validators |
|-------|-------|-------|---------------------|
| Simple | 1 | < 50 | ci + canonical (automated) |
| Moderate | 2-5 | 50-200 | ci + architecture + canonical |
| Complex | 5-15 | 200-500 | ci + architecture + security + canonical |
| Critical | 15+ or core | 500+ | All validators + canonical + human approval |

---

## Automated Scripts

| Script | Checks |
|--------|--------|
| `validate-ci.sh` | Build, test, lint, format, audit |
| `validate-tests.sh` | Unit, integration, coverage |
| `validate-operations.sh` | Tracing, cancellation, atomic writes |
| `validate-security.sh` | Secrets, injection, path traversal |
| `validate-architecture.sh` | Architecture patterns, dependencies |
| `validate-canonical.sh` | Canonical reference integrity, coverage, architecture sync |
| `validation-cache.sh` | Retry optimization |

---

## Workflows

### Standard Workflows

| Workflow | File | Use When |
|----------|------|----------|
| Feature Development | `prompts/feature-development.md` | New features |
| Bug Fix | `prompts/bug-fix.md` | Bug fixes |
| Emergency Hotfix | `prompts/hotfix.md` | Production issues |
| Refactoring | `prompts/refactoring.md` | Code improvement |
| Issue Implementation | `prompts/issue-implementation-series.md` | Batch implementation |

### Epic/Issue Management Workflows

| Workflow | File | Purpose |
|----------|------|---------|
| Epic Plan (Overview) | `prompts/epic-plan.md` | Cross-module epic planning across all architectures |
| Epic Plan (Module Slice) | `prompts/epic-plan.md` | Module-specific epic planning from architecture doc |
| Epic Plan (Free-Form) | `prompts/epic-plan.md` | Quick single-feature planning |
| Issue Draft | `prompts/issue-draft.md` | Create draft issues from approved epic |
| Git Issues | `prompts/git-issues.md` | Create epics/milestones + issues + tracking in GitHub/GitLab |
| Issue Closeout | `prompts/issue-closeout.md` | Verify AC → validators → canonical → compliance MR |
| Issue Merge | `prompts/issue-merge.md` | Merge MR → close issue → update tracking → close epic |
| Plan to Issues | `prompts/plan-to-issues.md` | Convert superpowers plan to GitHub/GitLab issues |

### Blueprint Management Workflows

| Workflow | File | Purpose |
|----------|------|---------|
| Blueprint Validate | `prompts/blueprint-validate.md` | Validate `.pi/` structure and integrity |
| Sync Check | `prompts/sync-check.md` | Verify exports match blueprint source |
| Context Refresh | `prompts/context-refresh.md` | Update context from codebase reality |
| Scope Analyzer | `prompts/scope-analyzer.md` | Auto-determine change scope + validators |
| Pattern Extract | `prompts/pattern-extract.md` | Extract patterns to `patterns.md` |
| Blueprint Update | `prompts/blueprint-update.md` | Reverse-sync implementation changes |

### Workflow Sequence

```
Blueprint Setup (one-time):
/blueprint-validate → /sync-check → [ready for implementation]

Multi-Module Planning (from scratch):
/epic-plan --overview → discover all modules, map dependencies, plan cross-module epics
  → /issue-draft (per epic in dependency order)
    → /git-issues → [implement] → /issue-closeout → /issue-merge

Single-Module Planning (targeted):
/epic-plan --module frontend docs/frontend-architecture.md
  → analyze gap, slice next epic, validate
  → /issue-draft → /git-issues → [implement] → /issue-closeout → /issue-merge

From Superpowers Plan:
/plan-to-issues → /git-issues → [implement] → /issue-closeout → /issue-merge
                      ↑                                          ↓
                      └────────────── next issue ────────────────┘

Maintenance:
/context-refresh → /pattern-extract → /blueprint-update → /sync-check → guardian generate
```

---

## Implementation Phase Requirements

**All implementation phases must:**

1. **Add canonical reference header** pointing to architecture module:
```typescript
/**
 * Canonical Reference: .pi/architecture/modules/[module].md#[section]
 * Implements: [spec from architecture doc]
 * Issue: #[issue-number]
 */
```

2. **Check architecture CHANGELOG** for recent changes affecting the module

3. **Reference specific sections** not just files: `.pi/architecture/modules/auth-system.md#token-validation`

---

## Validation Phase Requirements

**All validation phases must:**

1. **Run canonical validator**: `bash .pi/scripts/validate-canonical.sh`

2. **Check coverage**: Implementation files should have ≥50% canonical reference coverage

3. **Verify architecture sync**: Check if CHANGELOG has pending changes affecting implementation

4. **Validate accuracy**: References must point to existing architecture sections

5. **Report gaps**: Files without references flagged, architecture changes needing sync

---

## Architecture Change Workflow

When architecture changes:

1. **Update module doc**: `.pi/architecture/modules/[module].md`
2. **Add CHANGELOG entry**: `.pi/architecture/CHANGELOG.md`
3. **Identify impacted files**: List files needing canonical ref updates
4. **Notify via workflow**: Run `/blueprint-update` after implementation
5. **Validate sync**: Run `validate-canonical.sh` to verify updates

---

## Key Principles

1. **Template-driven** - Workflows in templates, not dynamic generation
2. **DAG-based** - Task nodes with dependencies, topological execution
3. **Minimal LLM** - LLM = planning tool only
4. **Bounded autonomy** - Hard caps on dynamic behavior
5. **Bounded retries** - Max 3 retries with exponential backoff + jitter (±25%)
6. **Risk-gated** - Safe=auto, Medium=confirm, Dangerous=dry-run
7. **Pre-validated** - Validator catches errors BEFORE execution
8. **Auditable** - Planning decisions tracked and diffable
9. **Architecture-traced** - All code linked to architecture documentation
10. **Change-log-governed** - Architecture changes tracked, migrations documented

---

## Generation Mappings

When running `guardian-framework generate`, `.pi/` files are transformed:

| Source | Destination | Transformation |
|--------|-------------|----------------|
| `AGENTS.md` | `.claude/CLAUDE.md`, `.opencode/context.md` | Direct copy + canonical header |
| `AGENTS.md` | `.github/copilot-instructions.md` | YAML frontmatter + canonical header |
| `architecture/modules/*.md` | `.claude/architecture/*.md` | Direct copy + canonical header |
| `architecture/CHANGELOG.md` | `.claude/architecture/CHANGELOG.md` | Direct copy |
| `skills/agents/*.md` | `.claude/agents/*.md`, `.github/agents/*.agent.md` | Direct copy + YAML frontmatter |
| `skills/validators/*.md` | `.opencode/prompts/*.txt` | Convert to .txt, compress |
| `context/*.md` | `.claude/context/*.md`, `.opencode/context/*.md` | Direct copy + canonical header |
| `context/*.md` | `.github/instructions/*.instructions.md` | YAML frontmatter + canonical header |
| `prompts/*.md` | `.opencode/workflows/*.md` | Nest under workflows/ |
| `scripts/*.sh` | `.pi/scripts/*.sh`, `.pi/scripts/*.sh` | Direct copy |
| `extensions/*.ts` | `extensions/*.ts` (pi only) | No export |
| `github/copilot/settings.json` | `.github/copilot/settings.json` | Direct copy |

---

## Language Patterns

Language-specific code patterns are stored in `templates/languages/`:
- `rust-patterns.md`
- `typescript-patterns.md`
- `python-patterns.md`
- `go-patterns.md`

Selected during `guardian init` based on project language.