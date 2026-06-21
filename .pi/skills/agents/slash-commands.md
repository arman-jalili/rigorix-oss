---
name: slash-commands
description: Slash command interception system for plan mode, workspace initialization, and prompt template injection.
model: inherit
tools: [Read]
---

# Slash Commands

Slash commands intercept user input starting with `/` (or `#` for snippet references) and transform them into structured actions before sending to the agent.

## Command Outcomes

| Outcome | Behavior |
|---------|----------|
| `handled` | Command ran; do NOT send a chat message |
| `send-prompt` | Replace user text with a template prompt, then send |
| `none` | Not a slash command; send as normal chat message |

## Built-in Commands

### /init — Initialize Workspace

**Outcome:** `send-prompt`

Sends a workspace scanning prompt that instructs the agent to:
1. Explore the codebase structure
2. Read key files (package.json, Cargo.toml, etc.)
3. Identify build/test/lint commands
4. Produce `TERAX.md` / project memory file at workspace root

**Prompt Template:**
```
Scan this workspace and produce a project memory file with:
- One-paragraph project description
- Build / test / dev commands
- Architecture overview (subsystems, data flow, key dirs)
- Conventions worth knowing
- Paths to entry points
Cap under 200 lines.
```

### /plan — Plan Mode Toggle

**Outcome:** `handled`

Toggles plan mode on/off:
- `/plan` or `/plan on` → Enable plan mode
- `/plan off` or `/plan exit` → Disable plan mode

### /validate — Run Validators

**Outcome:** `send-prompt`

Sends a validation prompt instructing the agent to run all configured validators.

### /scope — Classify Task Scope

**Outcome:** `send-prompt`

Sends a scope classification prompt to determine required validators.

## Command Format

```
/<command> [arguments]

Examples:
/plan on
/plan off
/init
/validate all
/scope implement auth module with OAuth2
```

## Command Marker Protocol

Commands are wrapped in XML markers for agent processing:

```xml
<terax-command name="init" />

[expanded prompt follows]
```

This allows the agent to recognize the command context in the conversation.

## Extending Commands

Add new commands by registering in the command registry:

```typescript
interface SlashCommand {
  name: string;           // Command name
  invocation: string;     // "/command"
  label: string;          // Display label
  icon: IconComponent;    // UI icon
  handler: (args: string) => SlashOutcome;
}
```

## Snippet References (#handle)

Input starting with `#` that matches a known snippet handle is treated as a snippet expansion:

```
#security-review — check the auth module
```

Expands to:
```
<snippet name="security-review">
[security review instructions]
</snippet>

check the auth module
```
