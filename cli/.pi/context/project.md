# Project Context

> Single source of truth for project-specific knowledge. All agents load this ONCE.
> **This is a template** — replace placeholders with actual values during `guardian init`.

## Project Overview

- **Name:** {{PROJECT_NAME}}
- **Version:** {{PROJECT_VERSION}}
- **Language:** rust
- **Type:** {{PROJECT_TYPE}}
- **Repository:** arman-jalili/rigorix-oss

## Core Principles

1. **Template-driven** - Workflows in templates, not dynamic generation
2. **DAG-based** - Task nodes with dependencies, topological execution
3. **Minimal LLM** - LLM = planning tool only
4. **Bounded autonomy** - Hard caps on dynamic behavior
5. **Bounded retries** - Max 3 retries with exponential backoff + jitter (±25%)
6. **Risk-gated** - Safe=auto, Medium=confirm, Dangerous=dry-run
7. **Pre-validated** - Validator catches errors BEFORE execution
8. **Auditable** - Planning decisions tracked and diffable

## Architecture

### Structure

```
{{PROJECT_NAME}}/
├── src/                 # Source code
├── tests/               # Test files
├── docs/                # Documentation
└── .pi/                 # Agent framework
```

### Key Patterns

- **Error handling:** {{ERROR_HANDLING_PATTERN}}
- **Tracing:** {{TRACING_PATTERN}}
- **Cancellation:** {{CANCELLATION_PATTERN}}
- **Atomic writes:** {{ATOMIC_WRITE_PATTERN}}

## Quality Gates

### Before Commit
```bash
{{BUILD_COMMAND}} && {{FORMAT_COMMAND}}
```

### Before Push
```bash
{{TEST_COMMAND}} && {{LINT_COMMAND}}
```

### Before Merge
```bash
{{TEST_COMMAND}}
{{LINT_COMMAND}}
{{FORMAT_CHECK_COMMAND}}
{{SECURITY_AUDIT_COMMAND}}
```

## Scope Classification

| Scope | Files | Lines | Required Validators |
|-------|-------|-------|---------------------|
| Simple | 1 | < 50 | ci-mr (automated) |
| Moderate | 2-5 | 50-200 | architecture-validator |
| Complex | 5-15 | 200-500 | architecture + security |
| Critical | 15+ or core | 500+ | All validators + human |

## Key Files

| File | Purpose |
|------|---------|
| docs/ARCHITECTURE.md | Architecture specification |
| {{KEY_FILE_1}} | {{KEY_FILE_1_PURPOSE}} |
| {{KEY_FILE_2}} | {{KEY_FILE_2_PURPOSE}} |

## Commands

| Command | Purpose |
|---------|---------|
| `{{BUILD_COMMAND}}` | Build project |
| `{{TEST_COMMAND}}` | Run tests |
| `{{LINT_COMMAND}}` | Lint check |
| `{{SECURITY_AUDIT_COMMAND}}` | Security audit |