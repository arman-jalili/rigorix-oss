---
name: angular-codegen
description: Minimal skill for Angular code generation with DDD + Clean Architecture. References full patterns on demand — never loads inline. Use when implementing Angular modules following enterprise patterns.
model: inherit
tools: [Read, Grep]
---

# Angular Code Generation — DDD + Clean Architecture Patterns

> Full reference: `.pi/skills/angular-enterprise-codegen.md`

## Quick Reference

| When you need... | Read this section |
|-----------------|-------------------|
| Module structure | Section 1 — 4-layer DDD for Angular |
| Domain entities | Section 2 — Pure business logic, no Angular |
| Application layer | Section 3 — Injectable services, Signal stores |
| Infrastructure | Section 4 — HttpClient wrappers |
| UI Components | Section 5 — Standalone components, control flow |
| Testing | Section 6 — TestBed patterns |

## DDD Layer Contract

| Layer | Purpose | Dependencies |
|-------|---------|-------------|
| `domain/` | Pure business logic | None (no Angular imports) |
| `application/` | Services, stores | `domain/` |
| `infrastructure/` | HTTP clients, adapters | `domain/` + `application/` |
| `interfaces/` | Components, pages | `application/` |

## Rules
- NEVER read full reference — read specific sections
- Domain layer has ZERO Angular imports
- Use `inject()` for DI, never constructor injection
- Use `signal()` over BehaviorSubject for state
