---
name: session-persistence
description: Structured session lifecycle with lazy-loaded message history, auto-derived titles, and per-session state isolation.
model: inherit
tools: [Read]
---

# Session Persistence

Sessions provide structured conversation lifecycle with lazy loading, auto-titling, and per-session state isolation.

## Session Storage Format

```json
{
  "sessions": [
    {
      "id": "s-1a2b3c-x7y8z9",
      "title": "Fix auth token refresh bug",
      "createdAt": 1714000000000,
      "updatedAt": 1714003600000
    }
  ],
  "activeId": "s-1a2b3c-x7y8z9"
}
```

### Per-Session Messages

Messages stored under key `messages:{sessionId}`:
```json
{
  "messages:s-1a2b3c-x7y8z9": [
    { "role": "user", "parts": [...] },
    { "role": "assistant", "parts": [...] }
  ]
}
```

## Session ID Format

```
s-{timestamp-base36}-{random-base36}
```

Example: `s-m5x7k2-a3b9c1`

## Auto-Derived Titles

Title is derived from the **first user message**, stripped of:
- Terminal context blocks
- Selection blocks
- File content blocks
- Truncated to 40 characters with ellipsis

## Lazy Loading

- **Boot:** Load only session list + active ID (single IPC roundtrip)
- **Open:** Load messages for the opened session on-demand
- **Memory:** Keep active session messages in a Map; hydrate others on switch

## Per-Session State

Each session maintains isolated state:
- **Todos:** Task list with progress tracking (`todos:{sessionId}`)
- **Shell session:** Persistent shell with cwd survival
- **Read cache:** File content cache invalidated per mutation
- **Agent metadata:** Current step, usage stats, model info

## Session Switching

When switching sessions:
1. Save current session messages
2. Load target session messages from store
3. Clear read cache (files may have changed)
4. Restore per-session todos
5. Update active session ID

## Auto-Save

Messages are saved to disk on every assistant turn completion.
Auto-save interval: 200ms debounce.

## Session Lifecycle

```
Create → Open → Interact (auto-save) → Switch (save + load) → ... → Delete
```

## Storage Keys

| Key | Content |
|-----|---------|
| `sessions` | Array of SessionMeta |
| `activeId` | Current session ID |
| `messages:{id}` | Message array for session |
| `todos:{id}` | Todo array for session |

## Title Derivation Algorithm

```typescript
function deriveTitle(messages): string {
  for (const m of messages) {
    if (m.role !== "user") continue;
    const text = m.parts
      .filter(p => p.type === "text")
      .map(p => p.text)
      .join(" ")
      .replace(/<terminal-context[\s\S]*?<\/terminal-context>/g, "")
      .replace(/<selection[\s\S]*?<\/selection>/g, "")
      .replace(/<file[\s\S]*?<\/file>/g, "")
      .trim();
    if (!text) continue;
    const first = text.split("\n")[0].trim();
    return first.length > 40 ? `${first.slice(0, 40)}…` : first;
  }
  return "New chat";
}
```
