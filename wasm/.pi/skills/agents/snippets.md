---
name: snippets
description: Reusable prompt fragments injected via #handle tokens. Compressed skill loading for on-demand context injection without full file reads.
model: inherit
tools: [Read]
---

# Snippets System

Snippets are reusable prompt fragments referenced by `#handle` tokens in chat. When the agent (or user) types `#handle`, the snippet body is expanded into an XML block and prepended to the message.

## Handle Format

- **Syntax:** `#handle` — lowercase alphanumeric with hyphens
- **Pattern:** `^[a-z0-9][a-z0-9-]*$`
- **Examples:** `#security-review`, `#rust-error-handling`, `#api-conventions`

## Expansion

`#handle` tokens are replaced with:

```xml
<snippet name="handle">
[snippet content]
</snippet>
```

Multiple handles expand to multiple `<snippet>` blocks, prepended to the user message. Unknown handles are left as-is (no error).

## Snippet Structure

```typescript
interface Snippet {
  id: string;            // Unique ID: sn-{timestamp}-{random}
  handle: string;        // The #handle used to reference it
  name: string;          // Human-readable name
  description: string;   // One-line description shown in autocomplete
  content: string;       // The snippet body (injected as XML)
}
```

## Use Cases

### 1. Skill Quick-Load

Instead of loading a full skill file, reference a snippet:

```
#security-review — check the auth module for vulnerabilities
```

### 2. Code Patterns

```
#rust-result-types — use Result<T, E> with thiserror, never unwrap()
```

### 3. Project Conventions

```
#naming-conventions — use snake_case for functions, PascalCase for types
```

### 4. Common Instructions

```
#no-comments — don't add comments unless the WHY is non-obvious
```

## Token Efficiency

| Approach | Tokens |
|----------|--------|
| Load full skill file | 500–2000 tokens |
| Reference snippet | 50–300 tokens |
| **Savings** | **70–90% reduction** |

## Snippet Categories

| Category | Handles | Purpose |
|----------|---------|---------|
| Security | `#security-review`, `#threat-model` | Security-specific instructions |
| Patterns | `#rust-errors`, `#ts-strict`, `#go-interfaces` | Language-specific patterns |
| Conventions | `#naming`, `#no-comments`, `#dry` | Project-wide conventions |
| Workflow | `#test-first`, `#commit-clean`, `#pr-body` | Process instructions |
| Quality | `#no-unwrap`, `#validate-input`, `#handle-errors` | Quality gates |

## Persistence

Snippets are persisted in `.pi/snippets.json` (JSON array) in the workspace root. They survive sessions and can be managed via `/snippet` commands.

## Slash Commands

| Command | Action |
|---------|--------|
| `/snippet list` | List all snippets with handles and descriptions |
| `/snippet add <handle>` | Create a new snippet interactively |
| `/snippet remove <handle>` | Delete a snippet |
| `/snippet edit <handle>` | Edit snippet content |
