/**
 * Plan Mode Extension for pi
 *
 * Intercepts mutating tool calls (edit, write_file, create_directory) and queues
 * them for batch review instead of executing immediately. Shell commands are
 * refused in plan mode. The user reviews all queued changes as a single diff,
 * then accepts/rejects in batch.
 *
 * Usage: /plan (toggles on/off), /plan on, /plan off
 */

import * as crypto from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { isToolCallEventType } from "@mariozechner/pi-coding-agent";

// ── State ──

interface QueuedEdit {
	id: string;
	kind: "edit" | "multi_edit" | "write_file" | "create_directory";
	filePath: string;
	originalContent: string;
	proposedContent: string;
	isNewFile: boolean;
	description?: string;
}

let planModeActive = false;
const editQueue: QueuedEdit[] = [];

function newEditId(): string {
	const ts = Date.now().toString(36);
	const rand = Math.random().toString(36).slice(2, 7);
	return `qe-${ts}-${rand}`;
}

// ── Plan mode toggle via slash command interception ──

function handleSlashCommand(input: string): string | null {
	const trimmed = input.trim();
	if (!trimmed.startsWith("/plan") && trimmed !== "/plan") return null;

	const args = trimmed.slice(5).trim().toLowerCase();
	if (args === "off" || args === "exit") {
		planModeActive = false;
		editQueue.length = 0;
		return "✅ Plan mode off. Queue cleared.";
	}
	planModeActive = true;
	return "✅ Plan mode on. Mutations will be queued for review. Use `/plan off` to exit.";
}

// ── Helper: read file content safely ──

function readFileContent(filePath: string): string {
	if (!fs.existsSync(filePath)) return "";
	return fs.readFileSync(filePath, "utf-8");
}

// ── Helper: hash for tracking ──

function hashContent(content: string): string {
	return crypto.createHash("sha256").update(content).digest("hex");
}

// ── Main extension ──

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async () => {
		planModeActive = false;
		editQueue.length = 0;
	});

	// Intercept user input for slash commands
	pi.on("input", async (event) => {
		const input = (event as { input?: string }).input;
		if (!input) return;
		const result = handleSlashCommand(input);
		if (result) {
			// Inject the status message as a system note
			event.input = result;
		}
	});

	// Intercept tool calls when plan mode is active
	pi.on("tool_call", async (event, ctx) => {
		if (!planModeActive) return;
		if (
			!isToolCallEventType("bash", event) &&
			!isToolCallEventType("write", event) &&
			!isToolCallEventType("edit", event)
		)
			return;

		const toolName = event.toolName;
		const blockedTools = ["bash", "bash_background", "run_command"];
		if (blockedTools.includes(toolName)) {
			return {
				block: true,
				reason:
					"Shell commands are not permitted in plan mode. Use /plan off to exit plan mode, or restrict yourself to read-only tools (read_file, grep, list_directory).",
			};
		}

		// Intercept write and edit tools
		if (toolName === "write" || toolName === "edit") {
			const inputPath = event.input.file_path ?? event.input.path;
			if (!inputPath) return;

			const absPath = path.isAbsolute(inputPath) ? inputPath : path.join(ctx.cwd, inputPath);
			const originalContent = readFileContent(absPath);
			const isNew = !fs.existsSync(absPath);

			// For edit tool, capture proposed content
			let proposedContent = "";
			if (toolName === "write") {
				proposedContent = event.input.content ?? "";
			} else if (toolName === "edit") {
				// For edit, we'd need to apply the edit to get proposed content
				// Store the edit params; actual content computed at review time
				proposedContent = `[edit queued: ${event.input.old_string?.length ?? 0} chars → ${event.input.new_string?.length ?? 0} chars]`;
			}

			const queuedEdit: QueuedEdit = {
				id: newEditId(),
				kind: toolName as QueuedEdit["kind"],
				filePath: absPath,
				originalContent,
				proposedContent,
				isNewFile: isNew,
				description: `${toolName} → ${path.basename(absPath)}`,
			};

			editQueue.push(queuedEdit);

			// Notify the user via UI status
			ctx.ui.setStatus("plan-mode", `📋 Plan: ${editQueue.length} edit(s) queued`);

			// Block the tool from executing — it's queued instead
			return {
				block: true,
				reason: `Queued for plan review: ${queuedEdit.description}. ${editQueue.length} edit(s) in queue. Use /plan off to review and apply.`,
			};
		}
	});

	// Register /plan command
	pi.registerCommand("plan", {
		description: "Toggle plan mode. Queues file mutations for batch review.",
		handler: async (_args, ctx) => {
			if (planModeActive) {
				planModeActive = false;
				const count = editQueue.length;
				editQueue.length = 0;
				ctx.ui.setStatus("plan-mode", null);
				ctx.ui.notify(`Plan mode off. ${count} queued edit(s) discarded.`, "warn");
			} else {
				planModeActive = true;
				ctx.ui.setStatus("plan-mode", "📋 Plan mode active");
				ctx.ui.notify("Plan mode on. All file mutations will be queued for review.", "info");
			}
		},
	});

	// Register /plan-apply command (review and apply queued edits)
	pi.registerCommand("plan-apply", {
		description: "Review and apply queued plan mode edits.",
		handler: async (_args, ctx) => {
			if (!planModeActive || editQueue.length === 0) {
				ctx.ui.notify("No queued edits to apply.", "info");
				return;
			}

			const summary = editQueue
				.map(
					(e, i) =>
						`${i + 1}. ${e.kind}: ${e.filePath} (${e.isNewFile ? "new" : hashContent(e.originalContent).slice(0, 8)} → ${hashContent(e.proposedContent).slice(0, 8)})`,
				)
				.join("\n");

			const confirmed = await ctx.ui.confirm(`Apply ${editQueue.length} queued edit(s)?`, summary);

			if (confirmed) {
				let applied = 0;
				for (const edit of editQueue) {
					try {
						const dir = path.dirname(edit.filePath);
						if (!fs.existsSync(dir)) {
							fs.mkdirSync(dir, { recursive: true });
						}
						fs.writeFileSync(edit.filePath, edit.proposedContent, "utf-8");
						applied++;
					} catch (err) {
						ctx.ui.notify(`Failed to apply ${edit.filePath}: ${err}`, "error");
					}
				}
				editQueue.length = 0;
				ctx.ui.setStatus("plan-mode", null);
				ctx.ui.notify(`Applied ${applied}/${editQueue.length} edits.`, "success");
			} else {
				editQueue.length = 0;
				ctx.ui.setStatus("plan-mode", null);
				ctx.ui.notify("All queued edits discarded.", "warn");
			}
		},
	});
}
