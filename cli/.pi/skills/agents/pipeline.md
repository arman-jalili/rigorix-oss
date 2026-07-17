---
name: pipeline
description: Multi-step workflow engine for iterating over items with per-step prompts and acceptance gates.
model: inherit
tools:
  - Read
  - Bash
  - Edit
---


# Pipeline Engine

A multi-step workflow that iterates over items (issues, tasks, etc.) with per-step prompts and acceptance conditions. Each step can require validators to pass before advancing.

## When to use it

Use `/pipeline` when you need to repeat a structured workflow across multiple items:

- "Close all P1 bugs" → implement → validate → create MR → merge
- "Add tests to these 5 modules" → implement tests → validate tests → commit
- "Security review for these 10 endpoints" → security-review → validate → document

Unlike `/goal` (single objective, auto-iterate), `/pipeline` is a **step-by-step state machine** with explicit gates.

## Commands

| Command | Effect |
|---------|--------|
| `/pipeline <name> --items "id1,id2,id3" --steps "implement,validate,create-mr"` | Start a pipeline |
| `/pipeline <name> --items "id1,id2" --steps "implement,validate" --merge-on-valid` | Start with auto-merge |
| `/pipeline` or `/pipeline status` | Show current pipeline progress |
| `/pipeline pause` | Pause at current step |
| `/pipeline resume` | Resume from where paused |
| `/pipeline skip-step` | Skip current step, move to next |
| `/pipeline retry-step` | Retry the current step |
| `/pipeline abort` | Kill pipeline |

## Built-in Steps

| Step | Prompt | Acceptance Gate |
|------|--------|-----------------|
| `implement` | `.pi/prompts/issue-implementation-series.md` | CI validator |
| `validate` | — | CI + tests + security validators |
| `create-mr` | `.pi/prompts/issue-closeout.md` | None |
| `merge` | — | CI + canonical validators |
| `document` | `.pi/prompts/blueprint-update.md` | Canonical validator |
| `test` | — | Tests validator |
| `security-review` | — | Security validator |

## Example: Close all P1 bugs

```
You: /pipeline "Close P1 bugs" --items "1234,1235,1236" --steps "implement,validate,create-mr,merge" --merge-on-valid

▶ Pipeline "Close P1 bugs" started (PL-0001)
Items: 1234, 1235, 1236
Steps: implement → validate → create-mr → merge
Merge on valid: enabled

[Step 1/12] Item 1234 → implement
  Agent: [loads issue-implementation-series.md, implements fix]
  ✓ CI passes → advance

[Step 2/12] Item 1234 → validate
  Agent: [runs ci, tests, security validators]
  ✓ All pass → advance

[Step 3/12] Item 1234 → create-mr
  Agent: [creates merge request]
  ✓ MR created → advance

[Step 4/12] Item 1234 → merge
  ✓ Merged → advance to next item

[Step 5/12] Item 1235 → implement
  ...continues
```

## Acceptance Gates

Each step can have a different acceptance condition:

| Type | Behavior |
|------|----------|
| `validator` | Runs specified validators. Must all pass to advance. |
| `shell` | Runs a custom shell command. Exit 0 = pass. |
| `llm` | LLM evaluates completion against a prompt. |
| `none` | No gate. Always advances. |

## Tools

| Tool | Description |
|------|-------------|
| `pipeline_status` | Show current pipeline status and progress |
| `pipeline_advance` | Mark current step as passed and advance |
| `pipeline_fail` | Mark current step as failed, skip remaining steps for this item |

## Behavioral Rules

→ Auto-advance: pipeline_advance (no asking)│

After completing any pipeline step, call `pipeline_advance` immediately without asking the user. The next step is already defined in the pipeline state — just advance and continue the loop.

## Pipeline States

```
running → paused (user) → running (user)
   ↓         ↓
 done      aborted (user)
   ↓
failed (step failure without retry)
```

## Custom Steps

You can add custom steps by name. Unknown steps have no prompt and no acceptance gate:

```
/pipeline "Custom flow" --items "task1,task2" --steps "implement,custom-review,validate"
```

The `custom-review` step will run with no prompt and no gate — the agent works freely.