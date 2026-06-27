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

For any non-trivial change, create a GitHub issue first. Use the issue template:

```bash
# Run the issue drafting workflow (see .pi/prompts/issue-draft.md)
```

### 3. Code Quality Standards

All code must pass before merging:

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

For complex features, follow the [Feature Development Workflow](.pi/prompts/feature-development.md):

1. **Coordinator** — Classifies scope, spawns validators
2. **Issue Creator** — Creates GitHub issue
3. **Validators** — Architecture + security validation
4. **Developer** — Implements against approved plan
5. **Post-Code Checks** — Automated validation scripts
6. **CI/MR** — Creates merge request

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
