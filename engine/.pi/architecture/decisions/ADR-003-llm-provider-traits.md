# ADR-003: LLM Provider Abstraction via Traits

**Status:** Accepted
**Date:** 2026-06-13
**Session:** 63c25384-1902-4b72-83bb-257f3f682af5

**Tech Stack:** Rust

## Context

The Planning Pipeline depends on three distinct LLM operations: intent classification, parameter extraction, and template generation. Multiple LLM providers exist (Anthropic Claude, OpenAI, DeepSeek, etc.) with different APIs. The system must support provider flexibility without coupling to any single API.

## Decision

Abstract each LLM operation behind an async trait, with separate implementations for each provider.

```rust
// Three independent traits
pub trait Classifier: Send + Sync { ... }
pub trait ParameterExtractor: Send + Sync { ... }
pub trait TemplateGenerator: Send + Sync { ... }

// Provider implementations
pub struct ClaudeClassifier;
pub struct OpenaiClassifier;
pub struct ClaudeParameterExtractor;
pub struct OpenaiParameterExtractor;
pub struct ClaudeTemplateGenerator;
pub struct OpenaiTemplateGenerator;
```

Provider selection is determined at startup from Config (`llm.provider` field + `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` env vars).

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **Trait-per-operation (chosen)** | Clean separation; testable with mocks; each trait evolves independently | More boilerplate (3 traits vs 1) | **Chosen** |
| **Single LLM trait** | Simpler to implement | Couples classification/extraction/generation; hard to test independently | Rejected — violates Single Responsibility |
| **Provider enum with match** | No dynamic dispatch | Every new provider requires core changes; can't test with mocks | Rejected — violates Open/Closed Principle |
| **Function pointers** | Minimal abstraction | Can't hold state (API key, model name, client) | Rejected — insufficient for real providers |

## Consequences

### Positive
- Providers are swappable at startup via config (`rigorix.toml` or `RIGORIX_MODEL` env var)
- Mock implementations (`MockClassifier`, `MockGenerator`) enable offline/CI mode
- Each provider implementation is isolated in its own file (classifier.rs, openai.rs)
- Budget tracking works uniformly across all provider calls

### Negative
- Requires async_trait crate (minor compile-time overhead)
- Three separate prompt engineering efforts (one per operation)

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/planning-pipeline.md`
- `.pi/architecture/modules/template-generation.md`

**Files to Update:**
- `rigorix/src/planning/classifier.rs` — Classifier trait + ClaudeClassifier + MockClassifier
- `rigorix/src/planning/extractor.rs` — ParameterExtractor trait + implementations
- `rigorix/src/planning/generator.rs` — TemplateGenerator trait + implementations
- `rigorix/src/planning/openai.rs` — OpenAI-compatible implementations

---

*Decision date: 2026-06-13*
