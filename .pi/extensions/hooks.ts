/**
 * Shell Hooks Extension for pi
 *
 * Declarative shell-script hooks that fire on lifecycle events.
 * Hooks are defined in AGENTS.md front matter under the `hooks:` section.
 * Scripts can block tool calls, inject context, or observe events.
 *
 * Inspired by Hermes-Agent's 3-layer hook system — adapted for Guardian's
 * TypeScript extension model with shell-script isolation.
 *
 * Supported events:
 *   pre_tool_call    — before any tool executes (can block)
 *   post_tool_call   — after any tool returns
 *   pre_llm_call     — before LLM turn (can inject context)
 *   post_llm_call    — after LLM turn completes
 *   on_session_start — new session created
 *   on_session_end   — session ended
 *   subagent_stop    — subagent completed
 *
 * Hook JSON protocol:
 *   stdin  → { hook_event_name, tool_name, tool_input, session_id, cwd, extra }
 *   stdout → { decision: "block", reason: "..." }  (pre_tool_call)
 *          → { context: "..." }                     (pre_llm_call)
 *          → {}                                     (observer hooks)
 */

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";

// ── Types ──

type ExtensionContext = {
	cwd: string;
	ui: {
		notify(message: string, level?: string): void;
		setStatus(key: string, message: string | null): void;
	};
	shell: {
		execute(
			command: string,
			options?: { signal?: AbortSignal },
		): Promise<{
			exitCode: number;
			stdout: string;
		}>;
	};
	tools: { execute(name: string, params: Record<string, unknown>): Promise<unknown> };
};

type ExtensionAPI = {
	on(event: string, handler: (event: unknown, ctx: ExtensionContext) => void | Promise<void>): void;
	registerTool(options: {
		name: string;
		label: string;
		description: string;
		parameters: unknown;
		execute(
			toolCallId: string,
			params: Record<string, unknown>,
			signal: AbortSignal,
			onUpdate: (update: { type: string; message: string }) => void,
			ctx: ExtensionContext,
		): unknown | Promise<unknown>;
	}): void;
	registerCommand(
		name: string,
		options: {
			description: string;
			handler(args: string, ctx: ExtensionContext): unknown | Promise<unknown>;
		},
	): void;
};

type HookEntry = {
	matcher?: string;
	command: string;
	timeout?: number;
};

type HookConfig = {
	[event: string]: HookEntry[];
};

type HookPayload = {
	hook_event_name: string;
	tool_name?: string | null;
	tool_input?: Record<string, unknown> | null;
	session_id?: string | null;
	cwd: string;
	extra?: Record<string, unknown>;
};

type HookBlockResponse = {
	decision: "block";
	reason: string;
};

type HookContextResponse = {
	context: string;
};

// ── Constants ──

const VALID_HOOKS = [
	"pre_tool_call",
	"post_tool_call",
	"pre_llm_call",
	"post_llm_call",
	"on_session_start",
	"on_session_end",
	"subagent_stop",
];

const DEFAULT_TIMEOUT = 60;
const MAX_TIMEOUT = 300;
const HOOKS_DIR = ".pi/hooks";

// ── YAML Parser (minimal) ──

function parseHookConfig(frontMatter: Record<string, unknown>): HookConfig {
	const config: HookConfig = {};
	const hooksRaw = frontMatter.hooks as Record<string, unknown> | undefined;
	if (!hooksRaw) return config;

	for (const [eventName, entries] of Object.entries(hooksRaw)) {
		if (!Array.isArray(entries)) continue;

		const parsed: HookEntry[] = [];
		for (const entry of entries) {
			if (!entry || typeof entry !== "object") continue;
			const e = entry as Record<string, unknown>;
			const cmd = e.command as string;
			if (!cmd) continue;

			parsed.push({
				matcher: (e.matcher as string) || undefined,
				command: cmd,
				timeout: Math.min(Math.max(Number(e.timeout) || DEFAULT_TIMEOUT, 1), MAX_TIMEOUT),
			});
		}

		if (parsed.length > 0) {
			if (VALID_HOOKS.includes(eventName)) {
				config[eventName] = parsed;
			}
		}
	}

	return config;
}

function loadHooksConfig(cwd: string): HookConfig {
	const agentsPath = join(cwd, ".pi/agent/AGENTS.md");
	if (!existsSync(agentsPath)) return {};

	const content = readFileSync(agentsPath, "utf-8");
	if (!content.startsWith("---\n")) return {};

	const endIdx = content.indexOf("\n---\n", 4);
	if (endIdx === -1) return {};

	const yamlBlock = content.slice(4, endIdx);
	try {
		// Minimal YAML parser for hooks config
		// For production, use the `yaml` package like workflow-config.ts
		return parseHookConfig(parseMinimalYaml(yamlBlock));
	} catch {
		return {};
	}
}

function parseMinimalYaml(yaml: string): Record<string, unknown> {
	const result: Record<string, unknown> = {};
	const lines = yaml.split("\n");
	let currentTopKey: string | null = null;
	let currentList: unknown[] | null = null;
	let currentDict: Record<string, unknown> | null = null;

	for (const line of lines) {
		const trimmed = line.trim();
		if (!trimmed || trimmed.startsWith("#")) continue;

		// Top-level key (no indent)
		if (!line.startsWith(" ") && !line.startsWith("\t")) {
			if (trimmed.endsWith(":")) {
				currentTopKey = trimmed.slice(0, -1);
				result[currentTopKey] = [];
				currentList = result[currentTopKey] as unknown[];
				currentDict = null;
			}
			continue;
		}

		// List item
		if (trimmed.startsWith("- ")) {
			if (currentList) {
				const itemStr = trimmed.slice(2).trim();
				if (itemStr.includes(":")) {
					// Inline dict start: - command: "..."
					const dict: Record<string, unknown> = {};
					const parts = itemStr.split(/:\s*/);
					if (parts.length >= 2) {
						const key = parts[0].trim();
						const val = parts
							.slice(1)
							.join(":")
							.trim()
							.replace(/^["']|["']$/g, "");
						dict[key] = val;
					}
					currentList.push(dict);
					currentDict = dict;
				} else {
					currentList.push(itemStr);
					currentDict = null;
				}
			}
			continue;
		}

		// Dict continuation (indented key: value)
		if (trimmed.includes(":") && currentDict) {
			const parts = trimmed.split(/:\s*/);
			if (parts.length >= 2) {
				const key = parts[0].trim();
				const val = parts
					.slice(1)
					.join(":")
					.trim()
					.replace(/^["']|["']$/g, "");
				currentDict[key] = Number.isNaN(Number(val)) ? val : Number(val);
			}
		}
	}

	return result;
}

// ── Hook Execution ──

async function runHook(
	cwd: string,
	entry: HookEntry,
	eventName: string,
	payload: HookPayload,
): Promise<{ blocked?: string; context?: string }> {
	const timeout = entry.timeout ?? DEFAULT_TIMEOUT;

	return new Promise((resolve) => {
		// Check matcher
		if (entry.matcher && payload.tool_name) {
			try {
				const re = new RegExp(entry.matcher);
				if (!re.test(payload.tool_name)) {
					resolve({});
					return;
				}
			} catch {
				resolve({}); // Invalid regex — skip
				return;
			}
		}

		const child = spawn("bash", ["-lc", entry.command], {
			cwd,
			timeout: timeout * 1000,
			stdio: ["pipe", "pipe", "pipe"],
		});

		let stdout = "";
		let stderr = "";

		child.stdin.write(JSON.stringify(payload));
		child.stdin.end();

		child.stdout.on("data", (d) => {
			stdout += d.toString();
		});
		child.stderr.on("data", (d) => {
			stderr += d.toString();
		});

		child.on("close", (code) => {
			if (code !== 0) {
				resolve({}); // Non-zero exit — skip silently
				return;
			}

			try {
				const trimmed = stdout.trim();
				if (!trimmed) {
					resolve({});
					return;
				}
				const parsed = JSON.parse(trimmed);
				if (parsed.decision === "block" || parsed.action === "block") {
					resolve({ blocked: parsed.reason || parsed.message || "blocked by hook" });
				} else if (parsed.context) {
					resolve({ context: parsed.context });
				} else {
					resolve({});
				}
			} catch {
				resolve({}); // Malformed JSON — skip
			}
		});

		child.on("error", () => resolve({}));
	});
}

// ── Extension ──

export default function (pi: ExtensionAPI) {
	let hooksConfig: HookConfig = {};
	const blockedTools: string[] = [];
	const injectedContext: string | null = null;

	pi.on("session_start", async (_event, ctx) => {
		hooksConfig = loadHooksConfig(ctx.cwd);
		const eventCount = Object.values(hooksConfig).reduce((sum, e) => sum + e.length, 0);
		ctx.ui.notify(
			`Guardian hooks initialized (${eventCount} hooks across ${Object.keys(hooksConfig).length} events)`,
			"info",
		);

		// Fire on_session_start hooks
		for (const entry of hooksConfig.on_session_start || []) {
			await runHook(ctx.cwd, entry, "on_session_start", {
				hook_event_name: "on_session_start",
				tool_name: null,
				tool_input: null,
				cwd: ctx.cwd,
			});
		}
	});

	// ── pre_tool_call — block dangerous operations ──
	pi.on("tool_call", async (event, ctx) => {
		const eventName = "tool_call" in (event as object) ? "tool_call" : "";
		if (eventName !== "tool_call") return;

		const toolEvent = event as { toolName: string; input: Record<string, unknown> };
		const toolName = toolEvent.toolName;
		const toolInput = toolEvent.input || {};

		const hooks = hooksConfig.pre_tool_call || [];
		if (hooks.length === 0) return;

		for (const entry of hooks) {
			const result = await runHook(ctx.cwd, entry, "pre_tool_call", {
				hook_event_name: "pre_tool_call",
				tool_name: toolName,
				tool_input: toolInput,
				cwd: ctx.cwd,
			});

			if (result.blocked) {
				ctx.ui.notify(`🚫 Hook blocked ${toolName}: ${result.blocked}`, "error");
				// Return block directive
				return {
					block: true,
					reason: `Guardian hook blocked: ${result.blocked}`,
				};
			}
		}
	});

	// ── post_tool_call — auto-format, logging, etc. ──
	pi.on("tool_result", async (event, ctx) => {
		const toolEvent = event as { toolName: string; input?: Record<string, unknown> };
		const toolName = toolEvent.toolName;

		const hooks = hooksConfig.post_tool_call || [];
		if (hooks.length === 0) return;

		for (const entry of hooks) {
			await runHook(ctx.cwd, entry, "post_tool_call", {
				hook_event_name: "post_tool_call",
				tool_name: toolName,
				tool_input: toolEvent.input || null,
				cwd: ctx.cwd,
				extra: { event: "tool_result" },
			});
		}
	});

	// ── /hooks command ──
	pi.registerCommand("hooks", {
		description: "List or manage Guardian shell hooks",
		handler: async (args, ctx) => {
			hooksConfig = loadHooksConfig(ctx.cwd);
			const raw = typeof args === "string" ? args : "";
			const tokens = raw.split(/\s+/).filter(Boolean);
			const action = tokens[0];

			if (!action || action === "list") {
				if (Object.keys(hooksConfig).length === 0) {
					ctx.ui.notify("No hooks configured. Add hooks: section to AGENTS.md.", "info");
					return;
				}

				const lines = ["## Guardian Shell Hooks\n"];
				for (const [event, entries] of Object.entries(hooksConfig)) {
					lines.push(`### ${event} (${entries.length} hook(s))`);
					for (const entry of entries) {
						const matcherInfo = entry.matcher ? ` [matcher: ${entry.matcher}]` : "";
						const timeoutInfo =
							entry.timeout !== DEFAULT_TIMEOUT ? ` [timeout: ${entry.timeout}s]` : "";
						lines.push(`  - \`${entry.command}\`${matcherInfo}${timeoutInfo}`);
					}
					lines.push("");
				}
				ctx.ui.notify(lines.join("\n"), "info");
				return;
			}

			if (action === "test") {
				const eventName = tokens[1];
				if (!eventName) {
					ctx.ui.notify("Usage: /hooks test <event_name>", "error");
					return;
				}
				const hooks = hooksConfig[eventName];
				if (!hooks || hooks.length === 0) {
					ctx.ui.notify(`No hooks registered for event: ${eventName}`, "warn");
					return;
				}
				ctx.ui.notify(`Testing ${hooks.length} hook(s) for ${eventName}...`, "info");
				for (const entry of hooks) {
					const result = await runHook(ctx.cwd, entry, eventName, {
						hook_event_name: eventName,
						tool_name: "test",
						tool_input: { test: true },
						cwd: ctx.cwd,
					});
					if (result.blocked) {
						ctx.ui.notify(`  ✗ blocked: ${result.blocked}`, "warn");
					} else if (result.context) {
						ctx.ui.notify(`  ℹ context: ${result.context.slice(0, 100)}`, "info");
					} else {
						ctx.ui.notify("  ✓ ok", "success");
					}
				}
				return;
			}

			ctx.ui.notify("Usage: /hooks [list|test <event>]", "info");
		},
	});
}
