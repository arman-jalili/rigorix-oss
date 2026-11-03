---
name: kanban
description: Durable task board for tracking multi-session, multi-agent work with state machine transitions.
model: inherit
tools: [Read, Bash]
---

# Guardian Kanban Board

A durable task board backed by `.pi/.guardian-kanban.json`. Tasks have state machines, dependency links, comments, and workspace management. Unlike subagent delegation (RPC-style, blocking), kanban is **fire-and-forget** and **crash-resilient**.

## Task States

```
triage → todo → ready → running → done → archived
                ↑         ↓         ↑
                └── blocked ────────┘
```

| State | Meaning |
|-------|---------|
| `triage` | New task, needs review |
| `todo` | Ready to be picked up |
| `ready` | All dependencies met, can start |
| `running` | Currently being worked on |
| `blocked` | Cannot proceed (reason recorded) |
| `done` | Completed |
| `archived` | Historical record |

## Tools

| Tool | Description |
|------|-------------|
| `kanban_create` | Create a new task (title, body, assignee, priority, dependencies) |
| `kanban_list` | List tasks, optionally filtered by status |
| `kanban_show` | Show full task details including comments |
| `kanban_complete` | Mark a task as done (auto-unblocks children) |
| `kanban_block` | Block a task with a reason |
| `kanban_comment` | Add a comment to a task |

## Commands

| Command | What it does |
|---------|-------------|
| `/kanban` or `/kanban status` | Board summary with task counts by status |
| `/kanban create <title>` | Quick-create a task |
| `/kanban list [status]` | List tasks (optionally filter by status) |

## Creating Tasks with Dependencies

```
kanban_create(
  title: "Implement auth middleware",
  body: "Add JWT validation to all /api/* routes",
  priority: "high",
  parents: ["TK-0001"]  # depends on TK-0001
)
```

When a parent task is marked `done`, dependent tasks automatically transition from `todo` to `ready`.

## Priority Levels

| Priority | Emoji | Use for |
|----------|-------|---------|
| `critical` | 🔴 | Blocking issues, security fixes |
| `high` | 🟠 | Current sprint, important features |
| `medium` | 🟡 | Normal features, improvements |
| `low` | 🟢 | Nice-to-have, tech debt |

## When to use Kanban vs. Subagents

| Use Kanban when... | Use Subagents when... |
|--------------------|----------------------|
| Work crosses sessions | Parent needs answer before continuing |
| Work needs human input | No humans involved |
| Work might be retried by different agent | Result goes back into parent's context |
| You need a durable audit trail | Short-lived investigation |

## Workspace Types

| Type | Description |
|------|-------------|
| `scratch` (default) | Temporary workspace, cleaned on completion |
| `dir:/absolute/path` | Shared directory (must be absolute path) |
