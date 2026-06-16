# ADR-009: LLM Provider Selection

**Status:** Accepted
**Date:** 2026-06-16

## Context

The Planning Pipeline needs an LLM for intent classification and parameter extraction. The Template Generation module needs an LLM for generating TOML templates from natural language.

## Decision

**Use Anthropic Claude via the Messages API** as the primary LLM provider.

The engine already has a `ClaudeTemplateGenerator` implementation:
- **Model**: `claude-sonnet-4-20250514` (default, configurable)
- **API**: `api.anthropic.com/v1/messages`
- **Max tokens**: 4096 (generation), configurable
- **Temperature**: 0.3 (low determinism for planning)
- **Retries**: Up to 3 with feedback on failures, exponential backoff on rate limits (429)

## Provider Selection Criteria

| Criterion | Score |
|-----------|-------|
| Code generation quality | High — sonnet-4 is strong at structured output |
| Structured output reliability | High — few cases of invalid TOML |
| API reliability | High — 99.9% uptime SLA |
| Latency | ~2-5s per call (acceptable for planning phase) |
| Cost | Reasonable for planning-only use (not execution) |

## Abstraction

The `TemplateGenerator` trait abstracts the LLM provider, making it possible to swap providers later:

```rust
#[async_trait]
pub trait TemplateGenerator: Send + Sync {
    async fn generate(
        &self,
        intent: &UserIntent,
        repo_context: &RepoContext,
        budget: &LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError>;
}
```

A future `OpenAIGenerator` or `LocalGenerator` can implement the same trait.

## Alternatives

| Provider | Reason Not Primary |
|----------|-------------------|
| OpenAI GPT-4o | Comparable quality, different API contract. Can be added via same trait. |
| Local model (LLaMA, etc.) | Lower quality for structured template generation. Deferred to v2 for offline mode. |
| Google Gemini | Less mature structured output handling. |

*Affects: Template Generation, Planning Pipeline*
