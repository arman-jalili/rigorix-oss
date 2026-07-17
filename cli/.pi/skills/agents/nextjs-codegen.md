---
name: nextjs-codegen
description: Minimal skill for Next.js code generation with DDD + Clean Architecture. References full patterns on demand — never loads inline. Use when implementing Next.js modules following enterprise patterns.
model: inherit
tools: [Read, Grep]
---

# Next.js Code Generation — DDD + Clean Architecture Patterns

> Full reference: `.pi/skills/nextjs-enterprise-codegen.md`

## Quick Reference

| When you need... | Read this section |
|-----------------|-------------------|
| Module structure | Section 1 — 4-layer DDD for Next.js |
| Domain entities | Section 2 — Pure business logic, no framework |
| Application layer | Section 3 — Server actions, Zustand stores |
| Infrastructure | Section 4 — API clients, storage adapters |
| UI Components | Section 5 — App Router pages, client components |

## DDD Layer Contract

| Layer | Purpose | Dependencies |
|-------|---------|-------------|
| `domain/` | Pure business logic | None (no Next.js imports) |
| `application/` | Server actions, stores | `domain/` |
| `infrastructure/` | API clients, adapters | `domain/` + `application/` |
| `interfaces/` | Pages, components | `application/` |

## Rules
- NEVER read full reference — read specific sections
- Domain layer has ZERO Next.js/React imports
- Use `'use client'` only in interfaces/ layer
