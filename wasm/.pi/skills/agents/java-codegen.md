---
name: java-codegen
description: Minimal skill for Java/Spring Boot code generation with DDD + Clean Architecture. References full patterns on demand — never loads inline. Use when implementing Java modules following enterprise patterns.
model: inherit
tools: [Read, Grep]
---

# Java/Spring Boot Code Generation — DDD + Clean Architecture Patterns

> Do NOT load the full reference document. Read specific sections when needed.
> Full reference: `.pi/skills/java-spring-enterprise-codegen.md`

## Quick Reference

| When you need... | Read this section |
|-----------------|-------------------|
| Module structure | Section 1 — 4-layer DDD layout + dependency direction |
| Domain entities | Section 2 — Entity.java, Value Object record, Repository interface |
| Error handling | Section 3 — DomainError, ErrorCode enum |
| Application services | Section 4 — @Service, interface + impl pattern |
| Repository impl | Section 5 — JPA EntityManager repository |
| REST controllers | Section 6 — @RestController, @ExceptionHandler |
| Testing | Section 7 — MockMvc, @SpringBootTest |
| Anti-patterns | Section 8 — what NOT to do |
| Project structure | Section 9 — Maven/Gradle module layout |

## DDD Layer Contract

| Layer | Purpose | Dependencies |
|-------|---------|-------------|
| `domain/` | Pure business logic | None (pure Java, no Spring) |
| `application/` | Use case orchestration | `domain/` |
| `infrastructure/` | External adapters | `domain/` + `application/` |
| `interfaces/` | API contracts | `application/` |

## Rules
- NEVER read full reference — read specific sections
- Use `grep` + `Read offset/limit` to target sections
- Follow the DDD layer structure — no `contracts/` wrapper
- Domain layer has ZERO Spring annotations
- Use constructor injection, NEVER field injection
- Use `record` for value objects (Java 16+)
