---
name: go-codegen
description: Minimal skill for Go code generation with DDD + Clean Architecture. References full patterns on demand — never loads inline. Use when implementing Go modules following enterprise patterns.
model: inherit
tools: [Read, Grep]
---

# Go Code Generation — DDD + Clean Architecture Patterns

> Do NOT load the full reference document. Read specific sections when needed.
> Full reference: `.pi/skills/go-enterprise-codegen.md`

## Quick Reference

| When you need... | Read this section |
|-----------------|-------------------|
| Module structure | Section 1 — 4-layer DDD layout + dependency direction |
| Domain entities | Section 2 — entity.go, value.go, repository interface |
| Error handling | Section 3 — *DomainError, ErrorCode, sentinel errors |
| Application services | Section 4 — Service interface + implementation |
| Repository impl | Section 5 — GORM/sql implementation pattern |
| HTTP handlers | Section 6 — Handler → Service → Domain flow |
| Configuration | Section 7 — envconfig/viper pattern |
| Testing | Section 8 — unit tests, repository tests |
| Anti-patterns | Section 9 — what NOT to do |
| Project structure | Section 10 — cmd/ internal/ pkg/ layout |

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
- Always define repository interfaces in `domain/`, implement in `infrastructure/`
- Always use typed `*DomainError`, never bare strings
