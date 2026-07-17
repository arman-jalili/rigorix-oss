---
name: design-system-codegen
description: Minimal skill for design system architecture with DDD + adapter pattern. References full patterns on demand — never loads inline. Use when building design system components, tokens, or CSS integration.
model: inherit
tools: [Read, Grep]
---

# Design System Code Generation — DDD + Adapter Pattern

> Full reference: `.pi/skills/design-system-enterprise-codegen.md`

## Quick Reference

| When you need... | Read this section |
|-----------------|-------------------|
| Structure overview | Section 1 — DDD module layout, framework agnostic |
| Design tokens | Section 2 — ColorToken, SpacingToken, TypeScale as domain values |
| Variant engine | Section 3 — defineVariants(), type-safe component props |
| CSS adapter (Tailwind) | Section 4 — Tailwind class generation from tokens |
| CSS adapter (CSS Modules) | Section 4 — CSS variable generation |
| CSS adapter (MUI/Material) | Section 4 — MUI theme mapping |
| Component pattern | Section 5 — Button component using application layer |
| Theming (dark mode) | Section 6 — ThemeService, data-theme attribute |
| Testing | Section 7 — token tests, variant engine tests |
| Choosing your stack | Section 8 — Tailwind vs CSS Modules vs MUI by use case |

## DDD Layer Contract for Design Systems

| Layer | Purpose | Example | CSS framework aware? |
|-------|---------|---------|---------------------|
| `domain/tokens/` | Typed design tokens | `ColorToken.PRIMARY` | No |
| `application/` | Variant resolution, theming | `defineVariants()` | No |
| `infrastructure/` | CSS adapter | `tailwind-adapter.ts` | **Yes** — this is the only layer |
| `interfaces/` | Rendered components | `button.tsx` | No (uses application layer) |

## Rules
- NEVER read full reference — read specific sections
- Design tokens are pure TypeScript — zero CSS, zero framework imports
- Components import from `application/`, never from `infrastructure/` directly
- The adapter is the **only file** that changes when you switch CSS frameworks
- Pick one adapter per project — never mix Tailwind and MUI in the same component
