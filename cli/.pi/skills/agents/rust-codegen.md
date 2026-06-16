---
name: rust-codegen
description: Minimal skill for Rust code generation. References full patterns on demand — never loads inline. Use when implementing Rust modules following enterprise patterns.
model: inherit
tools: [Read, Grep]
---

# Rust Code Generation — On-Demand Patterns

> Do NOT load the full reference document. Read specific sections below when needed.
> Full reference: `.pi/skills/rust-enterprise-codegen.md`

## Quick Reference

When implementing, read ONLY the section you need:

| When you need... | Read this section from the reference |
|-----------------|--------------------------------------|
| Module structure | Section 1 — Clean Architecture 4-layer layout + header template |
| Error types | Section 2 — thiserror enums, root error aggregation, is_retriable() |
| Secret handling | Section 3 — Secret value object with redacted Display |
| State machines | Section 4 — Typed enum with is_terminal(), transition methods |
| RAII guards | Section 5 — Budget reservation with Drop auto-release |
| Async patterns | Section 6 — JoinSet, CancellationToken, select!, bounded channels |
| Domain events | Section 7 — Tagged union with serde, helper methods |
| Configuration | Section 8 — Multi-source merging (flags > env > file > defaults) |
| Atomic file ops | Section 9 — write-tmp → fsync → rename pattern |
| Complex builders | Section 10 — Builder pattern + named constructors |
| Retry/backoff | Section 11 — BackoffStrategy, RetryDecision enum |
| EventBus | Section 12 — broadcast channel, in-memory log, drain() |
| Tests | Section 13 — AAA pattern, serde round-trip, proptest, concurrency |
| Documentation | Section 14 — Module header template, @canonical refs |
| Anti-patterns | Section 15 — what NOT to do |
| Dependencies | Section 16 — Cargo.toml conventions |
| HTTP retry | Section 17 — Transient vs fatal error handling |

## Command

```
# Read the section you need:
read .pi/skills/rust-enterprise-codegen.md
# Then grep for the specific pattern:
grep "section 6" .pi/skills/rust-enterprise-codegen.md
# Or read limited lines:
read .pi/skills/rust-enterprise-codegen.md --limit 60
```

## Rules

- NEVER read the full 30KB reference into context — read specific sections
- Target reads with `grep` + line numbers instead
- Each agent loads only the patterns it needs for its current task
