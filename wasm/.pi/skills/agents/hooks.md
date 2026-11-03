---
name: hooks
description: Shell-script hooks that fire on lifecycle events — block tools, inject context, observe.
model: inherit
tools: [Read]
---

# Shell Hook System

Declarative shell-script hooks that fire on lifecycle events. Scripts receive a JSON payload on stdin and can respond via stdout. Three hook systems compose together:

| System | Scope | Use Case |
|--------|-------|----------|
| **Shell hooks** | CLI + Gateway (via extension) | Drop-in scripts for blocking, formatting, context injection |
| **Plugin hooks** | pi extensions (TypeScript) | Deep integration — tool interception, metrics, guardrails |
| **Workspace hooks** | Guardian CLI operations | Generate/update lifecycle (`before_run`, `after_run`) |

## Supported Events

| Event | Fires When | Can Block? | Can Inject Context? |
|-------|-----------|------------|---------------------|
| `pre_tool_call` | Before any tool executes | Yes | No |
| `post_tool_call` | After any tool returns | No | No |
| `pre_llm_call` | Before LLM turn starts | No | Yes |
| `post_llm_call` | After LLM turn completes | No | No |
| `on_session_start` | New session created | No | No |
| `on_session_end` | Session ended | No | No |
| `subagent_stop` | Subagent completed | No | No |

## Configuration

Hooks are declared in AGENTS.md front matter:

```yaml
hooks:
  pre_tool_call:
    - command: "~/.pi/hooks/block-rm-rf.sh"
      matcher: "bash"
      timeout: 5
  post_tool_call:
    - command: "~/.pi/hooks/auto-format.sh"
      matcher: "write|edit"
  pre_llm_call:
    - command: "~/.pi/hooks/inject-git-status.sh"
```

## JSON Protocol

### stdin — Payload the script receives

```json
{
  "hook_event_name": "pre_tool_call",
  "tool_name": "bash",
  "tool_input": { "command": "rm -rf /" },
  "session_id": "sess_abc123",
  "cwd": "/home/user/project",
  "extra": { "event": "tool_call" }
}
```

### stdout — Optional response

```jsonc
// Block a pre_tool_call (both shapes accepted):
{"decision": "block", "reason": "Forbidden: rm -rf"}
{"action": "block", "message": "Forbidden: rm -rf"}

// Inject context for pre_llm_call:
{"context": "Today is Friday, 2026-04-17"}

// Silent no-op — empty or non-matching output is fine:
{}
```

## Example Hooks

### Block destructive `bash` commands

```bash
#!/usr/bin/env bash
# ~/.pi/hooks/block-rm-rf.sh
payload="$(cat -)"
cmd=$(echo "$payload" | jq -r '.tool_input.command // empty')
if echo "$cmd" | grep -qE 'rm[[:space:]]+-rf?[[:space:]]+/'; then
  printf '{"decision": "block", "reason": "blocked: rm -rf / is not permitted"}\n'
else
  printf '{}\n'
fi
```

### Auto-format after file writes

```bash
#!/usr/bin/env bash
# ~/.pi/hooks/auto-format.sh
payload="$(cat -)"
path=$(echo "$payload" | jq -r '.tool_input.file_path // empty')
[[ "$path" == *.ts ]] && command -v biome >/dev/null && biome check --write "$path" 2>/dev/null
printf '{}\n'
```

### Inject git status every turn

```bash
#!/usr/bin/env bash
# ~/.pi/hooks/inject-git-status.sh
cat - >/dev/null  # discard stdin
if status=$(git status --porcelain 2>/dev/null) && [[ -n "$status" ]]; then
  jq --null-input --arg s "$status" \
     '{context: ("Uncommitted changes:\n" + $s)}'
else
  printf '{}\n'
fi
```

## Commands

| Command | What it does |
|---------|-------------|
| `/hooks` or `/hooks list` | Show all registered hooks |
| `/hooks test <event>` | Test hooks for a specific event |
