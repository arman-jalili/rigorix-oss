/**
 * Snippets Extension for pi
 *
 * Manages reusable prompt fragments (snippets) that can be referenced via
 * #handle tokens in chat. When the agent or user types #handle, the snippet
 * body is expanded into an XML block prepended to the message.
 *
 * Handles:
 *   - #handle expansion in user input
 *   - /snippet list | add | remove | edit commands
 *   - Persistent storage in guardian-snippets.json
 */

import * as fs from "node:fs";
import * as path from "node:path";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

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
	const p = path.join(cwd, SNIPPETS_FILE);
	if (!fs.existsSync(p)) return [];
	try {
		return JSON.parse(fs.readFileSync(p, "utf-8")) as Snippet[];
	} catch {
		return [];
	}
}

function saveSnippets(cwd: string, snippets: Snippet[]): void {
	const p = path.join(cwd, SNIPPETS_FILE);
	const tmp = `${p}.tmp`;
	fs.writeFileSync(tmp, JSON.stringify(snippets, null, 2), "utf-8");
	fs.renameSync(tmp, p);
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

/** Expand #handle tokens in text, returning the body with tokens stripped and snippet XML blocks. */
function expandSnippetTokens(
	text: string,
	snippets: readonly Snippet[],
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
	return { body: body.replace(/[ \t]+\n/g, "\n").trim(), blocks };
}

export default function (pi: ExtensionAPI) {
	let snippets: Snippet[] = [];

	pi.on("session_start", async (_event, ctx) => {
		snippets = loadSnippets(ctx.cwd);
		if (snippets.length > 0) {
			pi.appendEntry({
				key: "guardian:snippets",
				value: snippets.map((s) => `#${s.handle} — ${s.description}`).join("\n"),
			});
		}
	});

	// Intercept input for #handle expansion
	pi.on("input", async (event, _ctx) => {
		const input = (event as { input?: string }).input;
		if (!input) return;
		const trimmed = input.trim();

		// Only expand #handle tokens (not slash commands)
		if (
			!trimmed.startsWith("#") ||
			(trimmed.startsWith("# ") === false && !trimmed.match(/^#[a-z0-9][a-z0-9-]*/i))
		)
			return;

		const { body, blocks } = expandSnippetTokens(input, snippets);
		if (blocks.length > 0) {
			event.input = [...blocks, body].join("\n\n");
		}
	});

	// Snippet management commands
	pi.registerCommand("snippet", {
		description: "Manage snippets: list, add, remove, edit",
		handler: async (args, ctx) => {
			snippets = loadSnippets(ctx.cwd); // reload
			const [subcmd, ...rest] = args;

			if (!subcmd || subcmd === "list") {
				if (snippets.length === 0) {
					ctx.ui.notify(
						"No snippets configured. Use /snippet add <handle> [name] to create one.",
						"info",
					);
				} else {
					const list = snippets
						.map((s) => `#${s.handle.padEnd(20)} ${s.name.padEnd(25)} ${s.description}`)
						.join("\n");
					ctx.ui.notify(
						`Snippets (${snippets.length}):\n\nHandle               Name                      Description\n${"─".repeat(72)}\n${list}`,
						"info",
					);
				}
				return;
			}

			if (subcmd === "add") {
				const rawHandle = rest[0];
				if (!rawHandle) {
					ctx.ui.notify("Usage: /snippet add <handle> [name]", "error");
					return;
				}
				const handle = normalizeHandle(rawHandle);
				if (!HANDLE_RE.test(handle)) {
					ctx.ui.notify(
						`Invalid handle: "${rawHandle}". Use lowercase letters, numbers, and hyphens.`,
						"error",
					);
					return;
				}
				if (snippets.some((s) => s.handle === handle)) {
					ctx.ui.notify(`Snippet #${handle} already exists. Use /snippet edit to modify.`, "warn");
					return;
				}
				const name = rest.slice(1).join(" ") || handle;
				const content = await ctx.ui.input(`Content for #${handle}`, "Enter snippet body...");
				if (!content) return;
				snippets.push({ id: newSnippetId(), handle, name, description: name, content });
				saveSnippets(ctx.cwd, snippets);
				ctx.ui.notify(`Snippet #${handle} created.`, "success");
				return;
			}

			if (subcmd === "remove") {
				const handle = normalizeHandle(rest[0] ?? "");
				const before = snippets.length;
				snippets = snippets.filter((s) => s.handle !== handle);
				if (snippets.length === before) {
					ctx.ui.notify(`Snippet #${handle} not found.`, "error");
				} else {
					saveSnippets(ctx.cwd, snippets);
					ctx.ui.notify(`Snippet #${handle} removed.`, "success");
				}
				return;
			}

			if (subcmd === "edit") {
				const handle = normalizeHandle(rest[0] ?? "");
				const existing = snippets.find((s) => s.handle === handle);
				if (!existing) {
					ctx.ui.notify(`Snippet #${handle} not found.`, "error");
					return;
				}
				const content = await ctx.ui.input(`New content for #${handle}`, existing.content);
				if (!content) return;
				existing.content = content;
				saveSnippets(ctx.cwd, snippets);
				ctx.ui.notify(`Snippet #${handle} updated.`, "success");
				return;
			}

			ctx.ui.notify(
				"Usage: /snippet list | /snippet add <handle> [name] | /snippet remove <handle> | /snippet edit <handle>",
				"info",
			);
		},
	});
}
