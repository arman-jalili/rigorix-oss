/**
 * Slash Commands Extension for pi
 *
 * Intercepts user input starting with `/` and transforms slash commands into
 * structured actions. Supports:
 *   /init   — Workspace initialization prompt
 *   /plan   — Plan mode toggle
 *   /validate — Run all validators
 *   /scope  — Classify task scope
 *   /snippet — Snippet management (#handle expansion)
 *
 * Also handles `#handle` token expansion for snippets.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

// ── Snippet types ──

interface Snippet {
	id: string;
	handle: string;
	name: string;
	description: string;
	content: string;
}

const SNIPPETS_FILE = "guardian-snippets.json";
const HANDLE_RE = /^[a-z0-9][a-z0-9-]*$/;

function loadSnippets(cwd: string): Snippet[] {
	const snippetsPath = path.join(cwd, SNIPPETS_FILE);
	if (!fs.existsSync(snippetsPath)) return [];
	try {
		return JSON.parse(fs.readFileSync(snippetsPath, "utf-8")) as Snippet[];
	} catch {
		return [];
	}
}

function saveSnippets(cwd: string, snippets: Snippet[]): void {
	const tmpPath = path.join(cwd, `${SNIPPETS_FILE}.tmp`);
	const finalPath = path.join(cwd, SNIPPETS_FILE);
	fs.writeFileSync(tmpPath, JSON.stringify(snippets, null, 2), "utf-8");
	fs.renameSync(tmpPath, finalPath);
}

function newSnippetId(): string {
	return `sn-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
}

function normalizeHandle(raw: string): string {
	return raw
		.trim()
		.toLowerCase()
		.replace(/\s+/g, "-")
		.replace(/[^a-z0-9-]/g, "")
		.replace(/-+/g, "-")
		.replace(/^-+|-+$/g, "");
}

// Expand #handle tokens in text
function expandSnippetTokens(
	text: string,
	snippets: Snippet[],
): { body: string; blocks: string[] } {
	const byHandle = new Map(snippets.map((s) => [s.handle, s]));
	const matched = new Map<string, Snippet>();

	const re = /(^|\s)#([a-z0-9][a-z0-9-]*)\b/gi;
	const body = text.replace(re, (full, lead: string, raw: string) => {
		const h = raw.toLowerCase();
		const snip = byHandle.get(h);
		if (!snip) return full;
		matched.set(snip.id, snip);
		return lead;
	});

	const blocks = Array.from(matched.values()).map(
		(s) => `<snippet name="${s.handle}">\n${s.content}\n</snippet>`,
	);
	return {
		body: body.replace(/[ \t]+\n/g, "\n").trim(),
		blocks,
	};
}

// ── Command definitions ──

const INIT_PROMPT = `Scan this workspace and produce a project memory file (.pi/PROJECT.md) with:
- One-paragraph project description
- Build / test / dev commands
- Architecture overview (subsystems, data flow, key dirs)
- Conventions worth knowing (naming, patterns, gotchas)
- Paths to entry points
Cap under 200 lines.`;

const VALIDATE_PROMPT = `Run all Guardian validators. For each validator:
1. Run the script: .pi/scripts/validate-<name>.sh
2. Report pass/fail with output
3. If any fail, suggest fixes
Validators to run: [check manifest for configured validators]`;

const SCOPE_PROMPT = `Classify the following task scope:
- Estimate files affected and lines changed
- Determine required validators (ci, architecture, security, etc.)
- Assess risk level (simple, moderate, complex, critical)

Task: {task_description}`;

// ── Main extension ──

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async () => {
		// Load snippets at session start
		const snippets = loadSnippets(process.cwd());
		if (snippets.length > 0) {
			pi.appendEntry({
				key: "guardian:snippets",
				value: snippets.map((s) => `#${s.handle} — ${s.description}`),
			});
		}
	});

	// Intercept user input for slash commands and snippet expansion
	pi.on("input", async (event, ctx) => {
		const input = (event as { input?: string }).input;
		if (!input) return;
		const trimmed = input.trim();

		// Handle #handle snippet expansion
		if (trimmed.startsWith("#")) {
			const snippets = loadSnippets(ctx.cwd);
			const { body, blocks } = expandSnippetTokens(input, snippets);
			if (blocks.length > 0) {
				event.input = [...blocks, body].join("\n\n");
				return;
			}
		}

		// Handle /command
		if (!trimmed.startsWith("/")) return;
		const [cmd, ...args] = trimmed.slice(1).split(/\s+/);
		const argStr = args.join(" ");

		switch (cmd) {
			case "init":
				event.input = INIT_PROMPT;
				ctx.ui.setStatus("command", "/init — workspace scan");
				break;

			case "validate":
				event.input = VALIDATE_PROMPT;
				ctx.ui.setStatus("command", "/validate — running validators");
				break;

			case "scope":
				event.input = SCOPE_PROMPT.replace("{task_description}", argStr || "current task");
				ctx.ui.setStatus("command", "/scope — classifying task");
				break;

			case "plan":
				// Plan mode is handled by plan-mode.ts extension
				ctx.ui.notify("Plan mode: use the plan-mode extension for /plan commands", "info");
				break;

			case "snippet": {
				const snippets = loadSnippets(ctx.cwd);
				if (args[0] === "list") {
					if (snippets.length === 0) {
						ctx.ui.notify(
							"No snippets configured. Use /snippet add <handle> to create one.",
							"info",
						);
					} else {
						const list = snippets
							.map((s) => `#${s.handle} — ${s.name}: ${s.description}`)
							.join("\n");
						ctx.ui.notify(`Snippets:\n\n${list}`, "info");
					}
				} else if (args[0] === "add" && args.length >= 2) {
					const handle = normalizeHandle(args[1]);
					if (!HANDLE_RE.test(handle)) {
						ctx.ui.notify(
							`Invalid handle: "${args[1]}". Use lowercase letters, numbers, and hyphens.`,
							"error",
						);
						break;
					}
					const name = args.slice(2).join(" ") || handle;
					const content = await ctx.ui.input("Snippet content", "Enter the snippet body...");
					if (!content) break;
					snippets.push({
						id: newSnippetId(),
						handle,
						name,
						description: name,
						content,
					});
					saveSnippets(ctx.cwd, snippets);
					ctx.ui.notify(`Snippet #${handle} created.`, "success");
				} else if (args[0] === "remove" && args.length >= 2) {
					const handle = normalizeHandle(args[1]);
					const before = snippets.length;
					const filtered = snippets.filter((s) => s.handle !== handle);
					if (filtered.length === before) {
						ctx.ui.notify(`Snippet #${handle} not found.`, "error");
					} else {
						saveSnippets(ctx.cwd, filtered);
						ctx.ui.notify(`Snippet #${handle} removed.`, "success");
					}
				} else {
					ctx.ui.notify(
						"Usage: /snippet list | /snippet add <handle> [name] | /snippet remove <handle>",
						"info",
					);
				}
				// Don't send to agent — command handled
				event.input = "(snippet command handled — no agent response needed)";
				break;
			}

			default:
				// Unknown command — let it through as normal text
				break;
		}
	});
}
