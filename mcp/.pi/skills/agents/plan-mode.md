---
name: plan-mode
description: Queues file mutations for batch review instead of executing immediately. User reviews all changes as a single diff before accepting/rejecting.
model: inherit
tools: [Read, Write, Edit]
---

# Plan Mode Extension

Plan Mode intercepts all mutating tool calls (`write_file`, `edit`, `multi_edit`, `create_directory`) and **queues** them for batch review instead of executing immediately. The agent works in a read-only research mode, then the user reviews all queued changes at once.

## Flow

1. User enables plan mode: `/plan` or `/plan on`
2. Agent reads files, researches, and **queues** mutations without executing
3. Agent stops and presents a summary of planned changes
4. User reviews all queued edits as a side-by-side diff
5. User accepts/rejects each change (or all at once)
6. Accepted changes are applied; rejected changes are discarded
7. Plan mode can stay on for iterative refinement

## Queued Edit Structure

```typescript
interface QueuedEdit {
  id: string;           // Unique edit ID (stable across updates)
  kind: "edit" | "multi_edit" | "write_file" | "create_directory";
  path: string;         // Absolute file path
  originalContent: string;
  proposedContent: string;
  isNewFile: boolean;   // true for write_file/create_directory on non-existent paths
  description?: string; // Optional human-readable description
}
```

## Behavior by Tool

| Tool | Normal Mode | Plan Mode |
|------|------------|-----------|
| `read_file` | Execute | Execute (read-only always runs) |
| `grep` | Execute | Execute |
| `list_directory` | Execute | Execute |
| `edit` | Execute immediately | Queue for review |
| `multi_edit` | Execute immediately | Queue for review |
| `write_file` | Execute immediately | Queue for review |
| `create_directory` | Execute immediately | Queue for review |
| `bash_run` | Execute (with approval) | **Refuse** — no shell in plan mode |
| `bash_background` | Execute (with approval) | **Refuse** — no shell in plan mode |

## Validation in Plan Mode

While plan mode is active, the agent should:
- Use `todo_write` to track planned changes before queueing
- Stop after queuing all mutations (don't continue acting)
- Provide a brief summary: files changed, lines added/removed, new files created

## Anti-Abuse

- Shell commands (`bash_run`, `bash_background`) are refused in plan mode
- Subagent spawning is allowed but subagents also inherit plan mode restrictions
- The queue is per-session — switching sessions clears the queue

## Pi Extension API

```typescript
// Plan mode state managed via Zustand-style store
const planModeStore = {
  active: boolean,
  queue: QueuedEdit[],
  enable() { this.active = true; },
  disable() { this.active = false; this.queue = []; },
  enqueue(edit: QueuedEdit) { this.queue.push(edit); },
  clear() { this.queue = []; },
  accept(ids: string[]) { /* Apply queued edits */ },
  reject(ids: string[]) { /* Discard queued edits */ },
};
```
