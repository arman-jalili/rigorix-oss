---
name: model-registry
description: Model capability scoring system with intelligence/speed/cost ratings and auto-model selection by task type.
model: inherit
tools: [Read]
---

# Model Registry

Classify models by capability scores to auto-select the right model for each task.

## Capability Dimensions

Each model is scored on a 1–5 scale:

| Dimension | Meaning |
|-----------|---------|
| **Intelligence** | Reasoning quality, code understanding, accuracy |
| **Speed** | Response time (5 = sub-second, 1 = multi-minute) |
| **Cost** | Price efficiency (5 = cheapest, 1 = most expensive) |

## Model Tags

| Tag | Purpose |
|-----|---------|
| `vision` | Can process images |
| `reasoning` | Extended thinking / chain-of-thought |
| `tools` | Supports tool/function calling |
| `coding` | Optimized for code generation |

## Auto-Selection by Task

| Task Type | Recommended Tier | Example Models |
|-----------|-----------------|----------------|
| Architecture planning | Intelligence 5 | Opus, GPT-5, Gemini Pro |
| Complex code changes | Intelligence 4–5 | Sonnet, GPT-4, Codex |
| Quick edits / fixes | Speed 4+ | GPT-4-mini, Haiku |
| Inline autocomplete | Speed 5 | GPT-nano, Cerebras, Groq |
| Bulk code review | Cost 4+ | Flash, Groq, Cerebras |
| Security audit | Intelligence 5 + reasoning | Opus, Grok Reasoning |
| File exploration | Speed 4+ | GPT-4-mini, Flash |
| Test writing | Intelligence 3–4 | Sonnet, GPT-4-mini |

## Context Windows

| Tier | Context Window | Models |
|------|---------------|--------|
| Large | 1M+ tokens | GPT-5, Gemini Pro/Flash, xAI Grok |
| Standard | 200K tokens | Claude Opus/Sonnet/Haiku |
| Small | 128K tokens | GPT-4 variants, DeepSeek |
| Minimal | 32K tokens | Local GGUF models |

## Pricing Tiers

| Tier | Input ($/M tokens) | Output ($/M tokens) |
|------|-------------------|--------------------|
| Premium | $5–15 | $15–75 |
| Standard | $1–3 | $5–15 |
| Budget | $0.1–0.5 | $0.4–2.5 |
| Local | $0 | $0 |

## Model Selection API

```typescript
interface ModelInfo {
  id: string;
  label: string;
  hint: string;            // Short display name
  description: string;     // One-line description
  capabilities: {
    intelligence: 1|2|3|4|5;
    speed: 1|2|3|4|5;
    cost: 1|2|3|4|5;
  };
  tags?: ("vision"|"reasoning"|"tools"|"coding")[];
  contextLimit: number;    // Approximate context window
}

function selectModel(task: string): ModelInfo {
  // Auto-select based on task complexity
}
```

## Provider Support

| Provider | Native SDK | Keyless |
|----------|-----------|---------|
| OpenAI | @ai-sdk/openai | No |
| Anthropic | @ai-sdk/anthropic | No |
| Google | @ai-sdk/google | No |
| xAI | @ai-sdk/xai | No |
| Groq | @ai-sdk/groq | No |
| Cerebras | @ai-sdk/cerebras | No |
| OpenRouter | @ai-sdk/openai-compatible | No |
| OpenAI-Compatible | @ai-sdk/openai-compatible | Yes |
| LM Studio | @ai-sdk/openai-compatible | Yes |
