---
name: context-compaction
description: Strategies for managing context window limits through intelligent elision and compaction.
model: inherit
tools: [Read]
---

# Context Compaction Strategy

When conversations grow large, apply these compaction strategies to stay within context limits.

## Budget-Aware Thresholds

| Context Usage | Action |
|---------------|--------|
| 0–55% | No compaction needed |
| 55–70% | Elide superseded read results |
| 70–90% | Elide all old tool results (keep last 24 messages) |
| 90%+ | Aggressive compaction — summarize early conversation |

## Superseded Read Elision

When a file has been **mutated** (edit, write_file, multi_edit) since it was last read, the old read result no longer provides useful context. Replace it with:

```
[elided to save context — see prior tool call in history]
```

**Rule:** Track mutation paths from tool calls. If a `read_file` result's path appears in any subsequent mutation tool call input, elide that read result.

## Tail Preservation

Never elide the **last 24 messages**. Recent context is always more valuable than distant history. Keep the conversation tail intact at all cost levels.

## System Message Protection

**Never** elide system messages. They contain operating principles, tool descriptions, and behavioral instructions that govern every turn.

## Read Cache Invalidation

After any mutation to a file:
1. Remove the file from your read cache
2. Next `read_file` on that path will return fresh content
3. If you read the same file twice without intervening edits, the second call returns `{unchanged: true}` — skip re-reading

## Token Budget Rules

- `read_file` defaults to first 2000 lines / 25KB — use `offset`/`limit` for large files
- One focused `grep` beats three `list_directory` calls
- Don't re-read files you already have in context unless they were modified
- Before 5+ tool calls in a row, write a one-line plan via `todo_write`

## Compact Format

When space is tight, prefer compact tool results:

```
# Verbose (200 tokens)
{
  "path": "/src/auth.ts",
  "content": "import { ... } ... [100 lines of code]"
}

# Compact (8 tokens)
{
  "path": "/src/auth.ts",
  "size": 2456,
  "lines": 120
}
```

Use compact format for: file listings, grep summaries, directory trees.
Use verbose format only when you need the actual content to reason about changes.
