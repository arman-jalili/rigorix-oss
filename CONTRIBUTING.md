# Contributing to Rigorix

We welcome contributions! This document outlines the development workflow, coding standards, and expectations.

---

## Code of Conduct

All contributors must adhere to the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Be respectful, inclusive, and constructive.

---

## Getting Started

```bash
# Clone the repo
git clone https://github.com/arman-jalili/rigorix-oss.git
cd rigorix-oss

# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace
```

---

## Development Workflow

### 1. Architecture-First

Before writing code, check the architecture docs in `.pi/architecture/`:

```bash
# Read the relevant module spec
cat engine/.pi/architecture/modules/<module>.md

# Check for pending architecture changes
cat .pi/architecture/CHANGELOG.md
```

All modules follow **Clean Architecture** with frozen contracts:
- `domain/` — Entities, value objects, trait interfaces
- `application/` — Service traits, DTOs, factory interfaces
- `infrastructure/` — Repository interfaces
- `interfaces/` — API contracts

### 2. Create an Issue

For any non-trivial change, start a [GitHub Discussion](https://github.com/arman-jalili/rigorix-oss/discussions) in the Ideas or Architecture category. The maintainers will scope it using the Guardian issue template (`.pi/prompts/issue-template.md`) and convert it to a tracked issue.

For a deeper understanding of how issues are structured, see:
- [Issue template](.pi/prompts/issue-template.md) — Guardian contract format
- [Issue drafting workflow](.pi/prompts/issue-draft.md) — Full drafting process

### 3. Code Quality Standards

All code must pass before merging:

```bash
# Full local CI (86 checks — lint, build, test, security, docs, integration)
bash .pi/scripts/local-ci.sh

# Or run specific stages
bash .pi/scripts/local-ci.sh --stage=lint      # formatting + clippy
bash .pi/scripts/local-ci.sh --stage=test      # cargo test + proofing scripts
bash .pi/scripts/local-ci.sh --stage=security  # audit + secret scan

# Quick mode (skip release builds)
bash .pi/scripts/local-ci.sh --quick
```

Individual checks:

```bash
# Lint — zero warnings required
cargo clippy --workspace

# Format — must match rustfmt
cargo fmt --check

# Tests — all must pass
cargo test --workspace

# Security audit
cargo audit
```

### 4. Git Commit Convention

```
<type>(<scope>): <description>

Types: feat, fix, chore, docs, refactor, test, ci
Scopes: engine, cli, actions, <.pi module>

Examples:
  feat(engine): add circuit breaker to audit service
  fix(cli): handle SIGTERM during config loading
  docs(engine): update orchestrator module comments
```

### 5. Feature Development

Every feature must be implemented through the **[Guardian Framework](https://github.com/arman-jalili/guardian-framework)** for consistency — **an architecture enforcement framework for AI-assisted development.** Guardian enforces architecture-first development: canonical module specs drive epics, epics drive scoped issues, and issues drive validated implementations. This ensures every change is traceable back to architecture and verified by proof scripts before merge.

For complex features, follow the [Feature Development Workflow](.pi/prompts/feature-development.md):

1. **Coordinator** — Classifies scope, spawns validators
2. **Issue Creator** — Creates GitHub issue
3. **Validators** — Architecture + security validation
4. **Developer** — Implements against approved plan
5. **Post-Code Checks** — Automated validation scripts
6. **CI/MR** — Creates merge request

### 6. Pull Request Requirements

Every PR must include **proof scripts** that verify the feature is implemented correctly and is acceptable for merge. Proof scripts are automated validation stages that run as part of CI/CD and `local-ci.sh`.

#### Proof script requirements

| Requirement | What it means |
|-------------|---------------|
| **Proof scripts exist** | Each affected bounded context must have a proofing script at `<crate>/.pi/scripts/ci/stage_<context>_proofing.sh` |
| **Proof scripts pass** | All proofing scripts must pass in `local-ci.sh --stage=test` and in CI/CD |
| **Issue proofing doc** | Each issue references its proofing strategy in `<crate>/.pi/issues/issue-proofing.md` |
| **Architecture traceability** | Implementation references canonical module specs and ADRs, verified by `validate-canonical.sh` |

#### Merge gate checklist

Before marking a PR as ready for review, confirm:

- [ ] `bash .pi/scripts/local-ci.sh` passes locally (all stages)
- [ ] CI/CD pipeline is green on the PR branch
- [ ] Proof scripts exist for every bounded context touched by the change
- [ ] Proof scripts pass and produce clear pass/fail output
- [ ] Architecture conformance verified (`validate-canonical.sh`, `validate-architecture.sh`)
- [ ] Security scan passes (`cargo audit` + secret scan)
- [ ] No hardcoded secrets, API keys, or tokens in any file

---

## Project Structure

```
rigorix-oss/
├── engine/         # Core library (28 bounded contexts)
│   ├── src/        # Rust source
│   └── .pi/        # Architecture docs, ADRs
├── cli/            # CLI binary + TUI
│   ├── src/
│   └── .pi/
├── actions/        # GitHub Action adapter
│   ├── src/
│   └── .pi/
├── .pi/            # Root-level architecture, prompts, scripts
└── .gitnexus/      # Code intelligence index
```

---

## Testing

```bash
# Run all tests
cargo test --workspace

# Run a specific crate's tests
cargo test -p rigorix-engine
cargo test -p rigorix-cli
cargo test -p rigorix-actions

# With live LLM calls (requires API key)
cargo test -p rigorix-engine --features live-tests

# Benchmarks
cargo bench -p rigorix-engine
```

---

## Architecture Documentation

Each crate has its own `.pi/architecture/` directory:

| Directory | Contents |
|-----------|----------|
| `modules/` | Detailed specs for each bounded context |
| `decisions/` | ADRs explaining key design choices |
| `diagrams/` | System context, data flow, deployment |

---

## Questions?

Open a [GitHub Discussion](https://github.com/arman-jalili/rigorix-oss/discussions) or reach out to the maintainers.
