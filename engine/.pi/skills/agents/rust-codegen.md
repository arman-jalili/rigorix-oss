---
name: rust-codegen
description: Minimal skill for Rust code generation with DDD + Clean Architecture. References full patterns on demand — never loads inline. Use when implementing Rust modules following enterprise patterns.
model: inherit
tools: [Read, Grep]
---

# Rust Code Generation — DDD + Clean Architecture Patterns

> Do NOT load the full reference document. Read specific sections below when needed.
> Full reference: `.pi/skills/rust-enterprise-codegen.md`

## Quick Reference

When implementing, read ONLY the section you need:

| When you need... | Read this section from the reference |
|-----------------|--------------------------------------|
| Module structure | Section 1 — Clean Architecture 4-layer layout + header template + dependency direction |
| Aggregate root | Section 2 — DDD aggregate root with command methods, entity encapsulation |
| Value objects | Section 2 — Immutable value objects with self-validation |
| Repository pattern | Section 2 — Interface + implementation separation, collection semantics |
| Domain events | Section 2 (tactical) + Section 8 — Tagged union, serde, correlation IDs |
| Error types | Section 3 — thiserror enums, root error aggregation, is_retriable(), #[source] |
| Secret handling | Section 4 — Secret value object with redacted Display |
| State machines | Section 5 — Typed enum with is_terminal(), transition methods, tracking entity |
| RAII guards | Section 6 — Budget reservation with Drop auto-release |
| Async patterns | Section 7 — JoinSet, CancellationToken, select!, bounded channels |
| Configuration | Section 9 — Multi-source merging (flags > env > file > defaults) |
| Atomic file ops | Section 10 — write-tmp → fsync → rename pattern |
| Complex builders | Section 11 — Builder pattern + named constructors |
| Retry/backoff | Section 12 — BackoffStrategy enum |
| EventBus | Section 13 — broadcast channel, in-memory log, drain() |
| Tests | Section 14 — AAA pattern, serde round-trip, proptest, concurrency |
| Documentation | Section 15 — Module header template, @canonical refs, public API docs |
| Anti-patterns | Section 16 — what NOT to do (anyhow, unwrap, sync Mutex across await, etc.) |
| Dependencies | Section 17 — Cargo.toml conventions |

## Command

```
# Read the section you need (use the Read tool with limit parameter):
Read file_path=".pi/skills/rust-enterprise-codegen.md" limit=60
# Or grep for a specific section header:
Grep pattern="## 7\. Async" path=".pi/skills/rust-enterprise-codegen.md"
# Then Read with offset/limit targeting the lines you need
```

## DDD Layer Contract

Each module follows this strict 4-layer structure:

| Layer | Purpose | Dependencies | Contains |
|-------|---------|-------------|----------|
| `domain/` | Pure business logic | None (serde, chrono, uuid only) | Entities, value objects, aggregates, domain events, repository traits |
| `application/` | Use case orchestration | `domain/` | Service traits, DTOs, factory interfaces |
| `infrastructure/` | External adapters | `application/` + `domain/` | Repository implementations, DB clients, HTTP clients |
| `interfaces/` | API contracts | `application/` | HTTP routes, request/response DTOs, event consumers |

**Dependency Rule:** Inner layers never depend on outer layers. `domain/` depends on nothing. `application/` depends on `domain/`. `infrastructure/` depends on `application/`. `interfaces/` depends on `application/`.

## Rules

- NEVER read the full reference document into context — read specific sections
- Target reads with `grep` + line numbers instead
- Each agent loads only the patterns it needs for its current task
- ALWAYS follow the 4-layer DDD structure — no `contracts/` wrapper
- ALWAYS define repository traits in `domain/`, implement in `infrastructure/`
- ALWAYS use typed error enums (thiserror), never String errors
- ALWAYS encapsulate aggregate state behind methods
