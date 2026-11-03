---
name: python-codegen
description: Minimal skill for Python code generation with DDD + Clean Architecture. References full patterns on demand — never loads inline. Use when implementing Python modules following enterprise patterns.
model: inherit
tools: [Read, Grep]
---

# Python Code Generation — DDD + Clean Architecture Patterns

> Do NOT load the full reference document. Read specific sections when needed.
> Full reference: `.pi/skills/python-enterprise-codegen.md`

## Quick Reference

| When you need... | Read this section |
|-----------------|-------------------|
| Module structure | Section 1 — 4-layer DDD layout + dependency direction |
| Domain entities | Section 2 — entity.py, value.py, repository ABC |
| Error handling | Section 3 — DomainError, ErrorCode enum |
| Application services | Section 4 — Service class |
| Repository impl | Section 5 — SQLAlchemy implementation pattern |
| API controllers | Section 6 — FastAPI route handlers, error mapping |
| Testing | Section 7 — pytest unit tests |
| Anti-patterns | Section 8 — what NOT to do |
| Project structure | Section 9 — src/ module layout |

## DDD Layer Contract

| Layer | Purpose | Dependencies |
|-------|---------|-------------|
| `domain/` | Pure business logic | None (stdlib only) |
| `application/` | Use case orchestration | `domain/` |
| `infrastructure/` | External adapters | `domain/` + `application/` |
| `interfaces/` | API contracts | `application/` |

## Rules
- NEVER read full reference — read specific sections
- Use `grep` + `Read offset/limit` to target sections
- Follow the DDD layer structure — no `contracts/` wrapper
- Use `@dataclass(frozen=True)` for value objects
- Use `ABC` for repository interfaces
