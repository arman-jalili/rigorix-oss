---
name: curator
description: Skill lifecycle management — track usage, detect stale skills, recommend archival.
model: inherit
tools: [Read]
---

# Skill Curator

Background maintenance for agent-created skills. Tracks usage (view, use, patch counts), detects stale/unused skills, and recommends consolidation or archival.

## What It Tracks

| Metric | When it increments |
|--------|-------------------|
| `use_count` | Skill is loaded into a conversation's prompt |
| `view_count` | Agent reads the skill file |
| `patch_count` | Skill content is modified via edit/write |

## Lifecycle States

```
active → stale (30 days unused) → archived (90 days unused)
```

- **Active:** Normal use, no warnings
- **Stale:** Unused for 30+ days — will archive at 90 days if still unused
- **Archived:** Moved to `.pi/skills/.archive/` — recoverable with `/curator restore`

## Protection

- **Bundled skills** (shipped with Guardian) are never subject to curator mutation
- **Pinned skills** are protected from both auto-transitions and agent deletion
- Pin with: `/curator pin <skill-name>`
- Unpin with: `/curator unpin <skill-name>`

## Commands

| Command | What it does |
|---------|-------------|
| `/curator` or `/curator status` | Show curator status with usage stats |
| `/curator review` | Run review pass (archives stale skills) |
| `/curator review --dry-run` | Preview review without mutations |
| `/curator pin <name>` | Protect a skill from archival |
| `/curator unpin <name>` | Remove protection |
| `/curator restore <name>` | Move archived skill back to active |

## Tools

| Tool | Description |
|------|-------------|
| `curator_review` | Run the curator review pass |
| `curator_pin` | Pin a skill |
| `curator_unpin` | Unpin a skill |

## Configuration

```yaml
curator:
  enabled: true
  stale_after_days: 30
  archive_after_days: 90
  auto_review: true
```

## Best Practices

1. **Review the first dry-run** — see exactly what the curator would propose before it runs for real
2. **Pin skills you rely on** — especially hand-authored skills for private workflows
3. **Use restore freely** — archived skills are always recoverable
4. **Monitor the stale list** — skills going stale are candidates for consolidation
