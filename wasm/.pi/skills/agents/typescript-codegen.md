---
name: typescript-codegen
description: Minimal skill for TypeScript/Node.js code generation with DDD + Clean Architecture. References full patterns on demand — never loads inline. Use when implementing TypeScript modules following enterprise patterns.
model: inherit
tools: [Read, Grep]
---

# TypeScript Code Generation — DDD + Clean Architecture Patterns

> Do NOT load the full reference document. Read specific sections when needed.
> Full reference: `.pi/skills/typescript-enterprise-codegen.md`

## Quick Reference

| When you need... | Read this section |
|-----------------|-------------------|
| Module structure | Section 1 — 4-layer DDD layout + dependency direction |
| Domain entities | Section 2 — entity.ts, value.ts, repository interface |
| Error handling | Section 3 — ErrorCode enum, DomainError class |
| Application services | Section 4 — Service interface + implementation |
| Repository impl | Section 5 — TypeORM implementation pattern |
| HTTP controllers | Section 6 — Express/Fastify router, error mapping |
| Testing | Section 7 — unit tests, integration tests |
| Anti-patterns | Section 8 — what NOT to do |
| Project structure | Section 9 — src/ module layout |

## DDD Layer Contract

| Layer | Purpose | Dependencies |
|-------|---------|-------------|
| `domain/` | Pure business logic | None (TS stdlib only) |
| `application/` | Use case orchestration | `domain/` |
| `infrastructure/` | External adapters | `domain/` + `application/` |
| `interfaces/` | API contracts | `application/` |

## Rules
- NEVER read full reference — read specific sections
- Use `grep` + `Read offset/limit` to target sections
- Follow the DDD layer structure — no `contracts/` wrapper
- Use readonly/mapped types for value objects (Omit, Partial, Readonly)
