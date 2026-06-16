/**
 * Read-Only Mode Extension for pi
 *
 * Hard-enforces a tiny tool allowlist: read, grep, find, ls.
 * Blocks every other tool call while enabled.
 *
 * Usage:
 *   /read-only        → toggle
 *   /read-only on     → enable
 *   /read-only off    → disable
 *   /read-only status → show current state
 */

import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import {
	createFindTool,
	createGrepTool,
	createLsTool,
	createReadTool,
} from "@mariozechner/pi-coding-agent";

const COMMAND_NAME = "read-only";
const STATUS_KEY = "read-only-mode";
const WIDGET_KEY = "read-only-mode";
const READ_ONLY_TOOL_NAMES = ["read", "grep", "find", "ls"] as const;

function getReadOnlyToolNames(pi: ExtensionAPI): string[] {
	const allToolNames = new Set(pi.getAllTools().map((tool) => tool.name));
	return READ_ONLY_TOOL_NAMES.filter((name) => allToolNames.has(name));
}

function updateUi(pi: ExtensionAPI, ctx: ExtensionContext, enabled: boolean): void {
	if (!enabled) {
		ctx.ui.setStatus(STATUS_KEY, undefined);
		ctx.ui.setWidget(WIDGET_KEY, undefined);
		return;
	}

	const tools = getReadOnlyToolNames(pi).join(", ");
	const theme = ctx.ui.theme;
	ctx.ui.setStatus(STATUS_KEY, theme.fg("accent", "🔒") + theme.fg("muted", " read-only"));
	ctx.ui.setWidget(WIDGET_KEY, [theme.fg("muted", `🔒 ${tools || "(none)"}`)]);
}

function applyReadOnlyTools(pi: ExtensionAPI): void {
	pi.setActiveTools(getReadOnlyToolNames(pi));
}

function restoreTools(pi: ExtensionAPI, toolsBefore?: string[]): void {
	const allToolNames = new Set(pi.getAllTools().map((tool) => tool.name));
	const toolNames = (toolsBefore ?? pi.getAllTools().map((tool) => tool.name)).filter((name) =>
		allToolNames.has(name),
	);
	pi.setActiveTools(toolNames);
}

export default function (pi: ExtensionAPI) {
	let enabled = false;
	let toolsBeforeReadOnly: string[] | undefined;

	// Register read-only variants of allowed tools (use session cwd)
	const readTool = createReadTool(process.cwd());
	pi.registerTool({
		...readTool,
		async execute(tc, params, signal, _up, ctx) {
			return createReadTool(ctx.cwd).execute(tc, params, signal, _up);
		},
	});

	const grepTool = createGrepTool(process.cwd());
	pi.registerTool({
		...grepTool,
		async execute(tc, params, signal, _up, ctx) {
			return createGrepTool(ctx.cwd).execute(tc, params, signal, _up);
		},
	});

	const findTool = createFindTool(process.cwd());
	pi.registerTool({
		...findTool,
		async execute(tc, params, signal, _up, ctx) {
			return createFindTool(ctx.cwd).execute(tc, params, signal, _up);
		},
	});

	const lsTool = createLsTool(process.cwd());
	pi.registerTool({
		...lsTool,
		async execute(tc, params, signal, _up, ctx) {
			return createLsTool(ctx.cwd).execute(tc, params, signal, _up);
		},
	});

	function enable(ctx: ExtensionContext): void {
		if (enabled) {
			updateUi(pi, ctx, enabled);
			ctx.ui.notify("Read-only mode is already enabled.", "info");
			return;
		}
		enabled = true;
		toolsBeforeReadOnly = pi.getActiveTools();
		applyReadOnlyTools(pi);
		updateUi(pi, ctx, enabled);
		ctx.ui.notify(
			`Read-only mode enabled. Tools: ${getReadOnlyToolNames(pi).join(", ") || "(none)"}.`,
			"info",
		);
	}

	function disable(ctx: ExtensionContext): void {
		if (!enabled) {
			updateUi(pi, ctx, enabled);
			ctx.ui.notify("Read-only mode is already disabled.", "info");
			return;
		}
		enabled = false;
		restoreTools(pi, toolsBeforeReadOnly);
		toolsBeforeReadOnly = undefined;
		updateUi(pi, ctx, enabled);
		ctx.ui.notify("Read-only mode disabled. Previous tool access restored.", "info");
	}

	pi.registerCommand(COMMAND_NAME, {
		description: "Toggle hard-enforced read-only mode",
		handler: async (args, ctx) => {
			const action = args.trim().toLowerCase();
			switch (action) {
				case "":
				case "toggle":
					toggle(ctx);
					return;
				case "on":
				case "enable":
					enable(ctx);
					return;
				case "off":
				case "disable":
					disable(ctx);
					return;
				case "status":
					updateUi(pi, ctx, enabled);
					ctx.ui.notify(
						enabled
							? `Read-only mode is ON. Allowed: ${getReadOnlyToolNames(pi).join(", ") || "(none)"}.`
							: "Read-only mode is OFF.",
						"info",
					);
					return;
				default:
					ctx.ui.notify(`Usage: /${COMMAND_NAME} [on|off|toggle|status]`, "warning");
			}
		},
	});

	pi.on("before_agent_start", async (event, ctx) => {
		if (!enabled) {
			updateUi(pi, ctx, enabled);
			return;
		}
		applyReadOnlyTools(pi);
		updateUi(pi, ctx, enabled);
		const tools = getReadOnlyToolNames(pi).join(", ") || "(none)";
		return {
			systemPrompt: `${event.systemPrompt}\n\n[Read-only mode is active]\n- You may only use these tools: ${tools}.\n- You must not attempt any action that changes local files, processes, git state, dependencies, databases, remote systems, or any other external state.\n- If the user asks for any write or side-effecting action, explain that read-only mode is enabled and tell them to run /${COMMAND_NAME} off first.`,
		};
	});

	pi.on("tool_call", async (event) => {
		if (!enabled) return;
		const allowed = new Set(getReadOnlyToolNames(pi));
		if (allowed.has(event.toolName)) return;
		return {
			block: true,
			reason: `Read-only mode is active. Tool "${event.toolName}" is blocked. Allowed: ${Array.from(allowed).join(", ") || "(none)"}. Use /${COMMAND_NAME} off to restore full tool access.`,
		};
	});

	pi.on("session_start", async (_event, ctx) => {
		if (enabled) applyReadOnlyTools(pi);
		updateUi(pi, ctx, enabled);
	});

	pi.on("session_switch", async (_event, ctx) => {
		if (enabled) applyReadOnlyTools(pi);
		updateUi(pi, ctx, enabled);
	});

	pi.on("session_fork", async (_event, ctx) => {
		if (enabled) applyReadOnlyTools(pi);
		updateUi(pi, ctx, enabled);
	});
}
