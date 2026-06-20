# rigorix-oss — Agent Workflow

This project uses **Guardian** for AI-assisted development.
Architecture-first: every change traces back to canonical architecture docs.

---

## Quick Start

1. Ensure `git init` has been run and `.gitignore` is in place (Guardian scaffolds a comprehensive `.gitignore`)
2. Fill in project context in `.pi/agent/AGENTS.md` (replace `[bracketed]` placeholders)
3. Run `npx guardian-framework generate` to create `.agents/`, `.claude/`, etc.

> **Multi-techstack repos:** If your repository has both backend and frontend (e.g., Java + TypeScript), run `guardian-framework init` in each directory with the same repository URL. Each gets its own `.pi/`, manifest, build config, and CI pipeline. Scope CI path triggers to avoid collisions (`backend/**`, `frontend/**`).

---

## Full Delivery Pipeline

```
/domain --explore "business intent"
    |  (agent fills exploration.md + glossary)
/domain --architect-scaffold <session-id>
    |  (generates modules, ADR-001, diagrams)
guardian project create --lang <lang>   (Epic 0 — greenfield only)
    |
/epic-plan --module <module>   (or /architect --epic "Name")
    |
/implement-series
```

Each step produces validated artifacts. See `.pi/context/domain-workflow.md` for details.

---

## For Agents

### Domain Discovery
| Command | Purpose |
|---------|---------|
| `/domain --explore "description"` | Start DDD domain exploration — agent fills exploration.md + glossary |
| `/domain --architect-scaffold <id>` | Generate architecture modules, ADR-001, diagrams from exploration |
| `/domain --validate <id>` | Validate exploration session structure |

### Architecture Planning
| Command | Purpose |
|---------|---------|
| `/architect --epic "Name" [--tracking-issue N]` | Start new epic from architecture modules |
| `/architect status` | Show current epic state |
| `/architect next-epic` | Find next logical slice to implement |
| `/architect abort` | Cancel current epic |
| `/epic-plan --overview` | Cross-module epic planning |
| `/epic-plan --module <name>` | Slice planning for a specific module |

### Implementation
| Command | Purpose |
|---------|---------|
| `/pipeline "Name" --items "..." --steps "..."` | Run implementation pipeline |
| `/goal <text> --validators=ci,tests` | Set standing goal with validators |

### Validation
| Command | Purpose |
|---------|---------|
| `bash .pi/scripts/ci/run_preflight.sh` | Run local preflight checks |
| `python .pi/scripts/ci/check_planning_packet.py --input=<packet>` | Validate planning packet |
| `python .pi/scripts/ci/validate_agent_output.py --input=<output> --schema=<type>` | Validate agent output |

### Maintenance
| Command | Purpose |
|---------|---------|
| `npx guardian-framework generate` | Refresh exports after editing `.pi/` |
| `npx guardian-framework update` | Pull framework updates |
| `npx guardian-framework info` | Show project status |

---

## Agent Definitions

Canonical agent definitions in `.pi/agents/` (6-section format: Purpose, Authority, Inputs, Outputs, DoD, Escalation):

| Agent | Role | Phase |
|-------|------|-------|
| [Architecture Coordinator](.pi/agents/architecture-coordinator.md) | Coordinator | A — Planning |
| [Issue Factory](.pi/agents/issue-factory.md) | Coordinator | B — Issue Generation |
| [Bootstrap Implementer](.pi/agents/bootstrap-implementer.md) | Builder | C — Implementation |
| [Architecture Validator](.pi/agents/architecture-validator.md) | Validator | D — Validation |
| [Security Validator](.pi/agents/security-validator.md) | Validator | D — Validation |
| [Operations Validator](.pi/agents/operations-validator.md) | Validator | D — Validation |

### 4-Phase Delivery Pipeline

```
Phase A: Planning       → Architecture Coordinator → planning packet
Phase B: Issue Gen      → Issue Factory            → issues with ACs
Phase C: Implement      → Bootstrap Implementer    → implemented issue
Phase D: Validation     → Architecture + Security + Operations → merge decision
```

## For Humans

- **`.gitignore` is pre-scaffolded** with language-agnostic defaults — extend for Python, Rust, Go, etc.
- **Edit `.pi/` files** to customize workflows, prompts, and skills
- **Run `npx guardian-framework update`** to pull framework updates
- **Run `npx guardian-framework generate`** after editing `.pi/`

### Directory Structure

```
.pi/                          ← Guardian source of truth
├── agent/AGENTS.md           ← Project context + runtime config
├── agents/                   ← Canonical agent definitions (6-section format)
├── architecture/             ← Canonical architecture modules + ADRs + diagrams
├── context/                  ← Shared knowledge (domain-workflow.md, etc.)
├── domain/                   ← Domain exploration + ubiquitous language
├── extensions/               ← Pi TypeScript extensions
├── prompts/                  ← Workflow templates
├── scripts/                  ← Validation + CI scripts (including deterministic checks)
└── skills/                   ← Agent definitions + validator skills
```

---

*Generated by Guardian Framework v1.2.0*
