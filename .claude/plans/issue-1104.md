# Issue #1104 — record whether MCP-only is the ceiling or a gap

**Branch:** `issue-1104-adapter-surface`

## Decision: it is the ceiling, and it is now stated
Framework-specific adapters (LangChain tools, OpenAI function wrappers, CrewAI, …) are **out of
scope**, not "not yet done". The reason is the same one that makes the engine worth adapting
*through*: an adapter that wraps the engine adds a surface where the guarantee can be lost, and adds
no guarantee of its own. The enforcement lives in the engine — preconditions, approval binding,
sequence policy, capacity, audit — and every host that speaks MCP reaches all of it through one
transport. A per-framework shim would be a second dispatch path with the engine's controls on the far
side of it, which is precisely the class (#1074) this series spent a round removing.

So: **MCP is the adapter surface.** Hosts outside it integrate by speaking MCP (server or stdio) or
by calling the library; neither is a framework adapter, and neither is planned.

## Where it is recorded
- root `README.md` — one line in `## Interfaces`, next to the other surfaces;
- `mcp/README.md` — what "supported host" means and what it does not;
- `mcp/.pi/architecture/modules/mcp-server.md` — the decision, with the reason.

## Why a sentence and not a code change
The issue says this explicitly: "The fix is a sentence, not a feature — unless the decision is that a
framework adapter is in scope." It is not, and an unstated scope boundary is read as an absence.
