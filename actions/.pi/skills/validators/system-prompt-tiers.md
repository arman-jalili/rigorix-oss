---
name: system-prompt-tiers
description: Tiered system prompt architecture. Uses full prompts for capable models and compressed prompts for fast/cheap models to optimize token usage.
model: inherit
tools: [Read]
---

# System Prompt Tiers

Different models have different capabilities, costs, and context needs. Use the right prompt tier for the right model to optimize both quality and token spend.

## Full Tier (Intelligence Score 4–5)

**For:** Claude Opus/Sonnet, GPT-4/5, Gemini Pro, Grok Reasoning

Complete system prompt with:
- Detailed environment description with `<env>` block format
- Operating principles (execute don't echo, chain actions, ask only when stuck)
- Full tool descriptions and budgets
- Editing rules (read-before-edit, exact string replacement)
- Path resolution rules
- Shell behavior (persistent sessions, background processes)
- Output style guidelines (terse, no filler)

## Lite Tier (Intelligence Score 1–3, Speed Score 4–5)

**For:** GPT-4o-mini, Claude Haiku, Gemini Flash, Cerebras, Groq fast models

Compressed system prompt (~40% of full size) with:
- Essential operating principles only
- Tool list without detailed descriptions
- Core editing rules (read-before-edit)
- Key path resolution rule
- Minimal output style guidance

## Auto-Selection Rules

| Model Type | Tier | Rationale |
|------------|------|-----------|
| Flagship / reasoning | Full | Benefits from detailed instructions |
| Balanced (Sonnet, GPT-4-mini) | Full | Handles context well, worth the tokens |
| Fast / cheap (Haiku, Flash, nano) | Lite | Token budget matters more than nuance |
| Ultra-fast (Cerebras, Groq LPU) | Lite | Sub-second responses need minimal prompt |
| Local / GGUF | Lite | Limited context, slower token throughput |

## Token Savings

| Model | Full Prompt | Lite Prompt | Savings |
|-------|------------|-------------|---------|
| GPT-4o-mini | ~1,200 tokens | ~450 tokens | ~750 tokens/turn |
| Claude Haiku | ~1,200 tokens | ~450 tokens | ~750 tokens/turn |
| Gemini Flash | ~1,200 tokens | ~450 tokens | ~750 tokens/turn |

At 20 turns per session, this saves **15,000 tokens** on fast models alone.

## Model Classification

```
Intelligence 5: Opus, GPT-5, Gemini Pro, Grok Reasoning, DeepSeek Pro
Intelligence 4: Sonnet, GPT-4, GPT-4-mini, Gemini Stable, Grok Fast
Intelligence 3: Haiku, GPT-nano, Flash, Cerebras, Groq, local GGUF
Intelligence 1-2: Tiny models, heavily quantized local models
```

## When to Override

- User explicitly requests a tier via `/prompt full` or `/prompt lite`
- Task complexity exceeds the current tier's capability (escalate to full)
- Token budget is critically low (downgrade to lite mid-session)
