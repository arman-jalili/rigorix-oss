// biome-ignore lint/suspicious/noExplicitAny: pi ExtensionContext has no published types
// biome-ignore lint/style/noNonNullAssertion: pi UI callbacks use non-null assertions
/**
 * File-Changes Extension for pi
 *
 * Tracks all files modified by pi during a session.
 * Provides /filechanges to review diffs, /filechanges-accept to keep them,
 * or /filechanges-decline to revert all pi-made changes.
 *
 * State persists across session branches via custom entries.
 */

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import type { ExtensionAPI, ExtensionCommandContext } from "@mariozechner/pi-coding-agent";
import {
	DynamicBorder,
	getMarkdownTheme,
	isEditToolResult,
	isToolCallEventType,
	isWriteToolResult,
} from "@mariozechner/pi-coding-agent";
import type { SelectItem } from "@mariozechner/pi-tui";
import { Container, Key, Markdown, SelectList, Text, matchesKey } from "@mariozechner/pi-tui";
// Custom session entry types
const ENTRY_BASELINE = "filechanges:baseline";
const ENTRY_CLEAR = "filechanges:clear";
const ENTRY_UNTRACK = "filechanges:untrack";

type Baseline = {
	path: string;
	absPath: string;
	originalContent: string | null; // null => file did not exist (created)
	createdAt: number;
};

type TrackedFile = {
	path: string;
	absPath: string;
	displayPath: string;
	originalContent: string | null;
	currentContent: string;
	diff: string;
	added: number;
	removed: number;
	kind: "new" | "edited";
	updatedAt: number;
};

function stripAtPrefix(p: string): string {
	return p.startsWith("@") ? p.slice(1) : p;
}

function normalizeToolPath(cwd: string, raw: string): { absPath: string; relPath: string } {
	const cleaned = stripAtPrefix(raw);
	const absPath = resolve(cwd, cleaned);
	const rel = relative(cwd, absPath);
	const relPath = rel && !rel.startsWith("..") && rel !== "" ? rel : cleaned;
	return { absPath, relPath };
}

async function readTextOrNull(absPath: string): Promise<string | null> {
	try {
		return await readFile(absPath, "utf-8");
	} catch {
		return null;
	}
}

function countDiffLines(unifiedDiff: string): { added: number; removed: number } {
	let added = 0;
	let removed = 0;
	for (const line of unifiedDiff.split("\n")) {
		if (line.startsWith("+++ ") || line.startsWith("--- ") || line.startsWith("@@")) continue;
		if (line.startsWith("+")) added++;
		else if (line.startsWith("-")) removed++;
	}
	return { added, removed };
}

// Simple unified diff generator — LCS-based. Zero external dependencies.
function createUnifiedPatch(
	oldName: string,
	newName: string,
	oldContent: string,
	newContent: string,
	contextSize = 3,
): string {
	const oldLines = oldContent.split("\n");
	const newLines = newContent.split("\n");
	const m = oldLines.length;
	const n = newLines.length;

	// Full LCS DP
	const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
	for (let i = 1; i <= m; i++) {
		for (let j = 1; j <= n; j++) {
			if (oldLines[i - 1] === newLines[j - 1]) dp[i][j] = dp[i - 1][j - 1] + 1;
			else dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
		}
	}

	// Backtrack
	const ops: Array<"equal" | "delete" | "insert"> = [];
	let i = m;
	let j = n;
	while (i > 0 || j > 0) {
		if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
			ops.push("equal");
			i--;
			j--;
		} else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
			ops.push("insert");
			j--;
		} else {
			ops.push("delete");
			i--;
		}
	}
	ops.reverse();

	// Build detailed changes with line content
	interface Change {
		type: "equal" | "delete" | "insert";
		oldLine?: string;
		newLine?: string;
	}
	const detailed: Change[] = [];
	let oi = 0;
	let ni = 0;
	for (const op of ops) {
		if (op === "equal") {
			detailed.push({ type: "equal", oldLine: oldLines[oi], newLine: newLines[ni] });
			oi++;
			ni++;
		} else if (op === "delete") {
			detailed.push({ type: "delete", oldLine: oldLines[oi] });
			oi++;
		} else {
			detailed.push({ type: "insert", newLine: newLines[ni] });
			ni++;
		}
	}

	// Group into hunks with context
	const hunks: Change[][] = [];
	let currentHunk: Change[] = [];
	let pendingContext: Change[] = [];

	for (const c of detailed) {
		if (c.type === "equal") {
			pendingContext.push(c);
			if (pendingContext.length > contextSize) {
				if (currentHunk.length > 0) {
					currentHunk.push(...pendingContext.slice(0, contextSize));
					hunks.push(currentHunk);
					currentHunk = [];
				}
				pendingContext = pendingContext.slice(contextSize);
			}
		} else {
			if (currentHunk.length === 0) {
				currentHunk = [...pendingContext.slice(-contextSize)];
			}
			currentHunk.push(c);
			pendingContext = [];
		}
	}
	if (currentHunk.length > 0) {
		currentHunk.push(...pendingContext.slice(0, contextSize));
		hunks.push(currentHunk);
	}

	if (hunks.length === 0) return "";

	// Build output
	const lines: string[] = [];
	lines.push(`--- ${oldName}`);
	lines.push(`+++ ${newName}`);

	let oldLineNum = 1;
	let newLineNum = 1;

	for (const hunk of hunks) {
		let oldCount = 0;
		let newCount = 0;
		if (hunk.every((c) => c.type === "equal")) continue;

		for (const c of hunk) {
			if (c.type === "delete" || c.type === "equal") oldCount++;
			if (c.type === "insert" || c.type === "equal") newCount++;
		}

		const hunkOldStart = oldLineNum;
		const hunkNewStart = newLineNum;
		lines.push(`@@ -${hunkOldStart},${oldCount} +${hunkNewStart},${newCount} @@`);

		for (const c of hunk) {
			if (c.type === "equal") {
				lines.push(` ${c.oldLine ?? ""}`);
				oldLineNum++;
				newLineNum++;
			} else if (c.type === "delete") {
				lines.push(`-${c.oldLine ?? ""}`);
				oldLineNum++;
			} else {
				lines.push(`+${c.newLine ?? ""}`);
				newLineNum++;
			}
		}
	}

	return `${lines.join("\n")}\n`;
}

function patchFromBaseline(displayPath: string, original: string | null, current: string): string {
	return createUnifiedPatch(displayPath, displayPath, original ?? "", current);
}

async function ensureParentDir(absPath: string): Promise<void> {
	await mkdir(dirname(absPath), { recursive: true });
}

export default function (pi: ExtensionAPI) {
	// In-memory state (reconstructed on session_start from custom entries)
	const baselines = new Map<string, Baseline>();
	const tracked = new Map<string, TrackedFile>();

	// Per-tool-call snapshot, committed on successful tool_result
	const pendingByToolCallId = new Map<
		string,
		{ path: string; absPath: string; before: string | null }
	>();

	function formatStatus(theme?: any): string | undefined {
		if (tracked.size === 0) return undefined;
		let edited = 0;
		let created = 0;
		for (const t of tracked.values()) {
			if (t.kind === "new") created++;
			else edited++;
		}
		if (!theme) return `Δ ${edited}  + ${created}`;
		return theme.fg("muted", `Δ ${edited}  + ${created}`);
	}

	function updateUi(ctx: any) {
		if (!ctx?.hasUI) return;
		ctx.ui.setStatus("filechanges", formatStatus(ctx.ui.theme));
	}

	async function recomputeTrackedFile(ctx: any, relPath: string) {
		const baseline = baselines.get(relPath);
		if (!baseline) return;

		const current = await readTextOrNull(baseline.absPath);
		const displayPath = baseline.path;

		if (baseline.originalContent === null) {
			// file was created
			if (current === null) {
				tracked.delete(relPath);
				return;
			}
			const diff = patchFromBaseline(displayPath, null, current);
			const { added, removed } = countDiffLines(diff);
			tracked.set(relPath, {
				path: baseline.path,
				absPath: baseline.absPath,
				displayPath,
				originalContent: null,
				currentContent: current,
				diff,
				added,
				removed,
				kind: "new",
				updatedAt: Date.now(),
			});
			return;
		}

		if (current === null) {
			const diff = patchFromBaseline(displayPath, baseline.originalContent, "");
			const { added, removed } = countDiffLines(diff);
			tracked.set(relPath, {
				path: baseline.path,
				absPath: baseline.absPath,
				displayPath,
				originalContent: baseline.originalContent,
				currentContent: "",
				diff,
				added,
				removed,
				kind: "edited",
				updatedAt: Date.now(),
			});
			return;
		}

		if (current === baseline.originalContent) {
			tracked.delete(relPath);
			return;
		}

		const diff = patchFromBaseline(displayPath, baseline.originalContent, current);
		const { added, removed } = countDiffLines(diff);
		tracked.set(relPath, {
			path: baseline.path,
			absPath: baseline.absPath,
			displayPath,
			originalContent: baseline.originalContent,
			currentContent: current,
			diff,
			added,
			removed,
			kind: "edited",
			updatedAt: Date.now(),
		});
	}

	async function clearLog(ctx: any, reason: "accept" | "decline") {
		baselines.clear();
		tracked.clear();
		pendingByToolCallId.clear();
		pi.appendEntry(ENTRY_CLEAR, { timestamp: Date.now(), reason });
		updateUi(ctx);
	}

	async function declineAll(ctx: any) {
		await ctx.waitForIdle();

		if (tracked.size === 0) {
			if (ctx.hasUI) ctx.ui.notify("filechanges: nothing to decline.", "info");
			return;
		}

		const args: string[] = ctx.args ?? [];
		const force = args.includes("force");

		if (ctx.hasUI && !force) {
			const ok = await ctx.ui.confirm(
				"Decline pi changes?",
				"This will revert ALL currently logged pi changes (overwrite files / delete created files).",
			);
			if (!ok) return;
		} else if (!ctx.hasUI && !force) {
			throw new Error("Decline requires confirmation. Run: /filechanges-decline force");
		}

		const items = [...tracked.values()].sort((a, b) => b.updatedAt - a.updatedAt);
		let reverted = 0;
		const errors: string[] = [];

		for (const item of items) {
			try {
				if (item.originalContent === null) {
					await rm(item.absPath, { force: true });
				} else {
					await ensureParentDir(item.absPath);
					await writeFile(item.absPath, item.originalContent, "utf-8");
				}
				reverted++;
			} catch (e: any) {
				errors.push(`${item.displayPath}: ${e?.message ?? String(e)}`);
			}
		}

		await clearLog(ctx, "decline");

		if (ctx.hasUI) {
			if (errors.length === 0) {
				ctx.ui.notify(`filechanges: declined changes for ${reverted} file(s).`, "success");
			} else {
				ctx.ui.notify(
					`filechanges: declined with ${errors.length} error(s). Run /filechanges to inspect.`,
					"warning",
				);
			}
		}
	}

	async function acceptAll(ctx: any) {
		await ctx.waitForIdle();

		if (tracked.size === 0) {
			if (ctx.hasUI) ctx.ui.notify("filechanges: nothing to accept.", "info");
			return;
		}

		const args: string[] = ctx.args ?? [];
		const force = args.includes("force");

		if (ctx.hasUI && !force) {
			const ok = await ctx.ui.confirm(
				"Accept pi changes?",
				"This will keep current files as-is and clear the modification log.",
			);
			if (!ok) return;
		} else if (!ctx.hasUI && !force) {
			throw new Error("Accept requires confirmation. Run: /filechanges-accept force");
		}

		const count = tracked.size;
		await clearLog(ctx, "accept");
		if (ctx.hasUI) ctx.ui.notify(`filechanges: accepted changes for ${count} file(s).`, "success");
	}

	async function rebuildFromSession(ctx: any): Promise<void> {
		baselines.clear();
		tracked.clear();
		pendingByToolCallId.clear();

		for (const entry of ctx.sessionManager.getBranch()) {
			if (entry.type !== "custom") continue;

			if (entry.customType === ENTRY_CLEAR) {
				baselines.clear();
				tracked.clear();
				continue;
			}

			if (entry.customType === ENTRY_BASELINE) {
				const data = entry.data as { path?: string; originalContent?: string | null };
				if (!data?.path) continue;
				const { absPath, relPath } = normalizeToolPath(ctx.cwd, data.path);
				baselines.set(relPath, {
					path: relPath,
					absPath,
					originalContent: typeof data.originalContent === "string" ? data.originalContent : null,
					createdAt: Date.now(),
				});
				continue;
			}

			if (entry.customType === ENTRY_UNTRACK) {
				const data = entry.data as { path?: string };
				if (!data?.path) continue;
				const { relPath } = normalizeToolPath(ctx.cwd, data.path);
				baselines.delete(relPath);
				tracked.delete(relPath);
			}
		}

		for (const relPath of baselines.keys()) {
			await recomputeTrackedFile(ctx, relPath);
		}

		updateUi(ctx);
	}

	// ── Commands ──

	pi.registerCommand("filechanges", {
		description: "Show files changed by pi and inspect diffs",
		handler: async (_args: string, ctx: ExtensionCommandContext) => {
			(ctx as any).args = _args ? _args.split(/\s+/g).filter(Boolean) : [];
			await ctx.waitForIdle();
			updateUi(ctx);

			if (!ctx.hasUI) {
				const items = [...tracked.values()].sort((a, b) => b.updatedAt - a.updatedAt);
				if (items.length === 0) {
					console.log("filechanges: no pi-made modifications recorded.");
					return;
				}
				for (const t of items.slice(0, 10)) {
					console.log(
						`${t.kind === "new" ? "+" : "Δ"} ${t.displayPath} (+${t.added}/-${t.removed})`,
					);
				}
				return;
			}

			while (true) {
				await ctx.waitForIdle();
				const items = [...tracked.values()].sort((a, b) => b.updatedAt - a.updatedAt);
				if (items.length === 0) {
					ctx.ui.notify("filechanges: no pi-made modifications recorded.", "info");
					return;
				}

				const selectItems: SelectItem[] = [
					{ value: "__accept__", label: "Accept changes", description: "Keep current files" },
					{ value: "__decline__", label: "Undo changes", description: "Restore original contents" },
					{ value: "__sep__", label: "────────", description: "" },
					...items.map((t) => ({
						value: t.path,
						label: `${t.kind === "new" ? "+" : "Δ"} ${t.displayPath}`,
						description: `+${t.added}/-${t.removed}`,
					})),
				];

				const picked = await ctx.ui.custom<string | null>(
					(tui, theme, _kb, done) => {
						const container = new Container();
						container.addChild(new DynamicBorder((s) => theme.fg("accent", s)));
						container.addChild(new Text(theme.fg("accent", theme.bold("File changes")), 1, 0));

						const list = new SelectList(selectItems, Math.min(14, selectItems.length), {
							selectedPrefix: (t) => theme.fg("accent", t),
							selectedText: (t) => theme.fg("accent", t),
							description: (t) => theme.fg("muted", t),
							scrollInfo: (t) => theme.fg("dim", t),
							noMatch: (t) => theme.fg("warning", t),
						});

						list.onSelect = (item) => {
							if (item.value !== "__sep__") done(item.value);
						};
						list.onCancel = () => done(null);
						container.addChild(list);
						container.addChild(
							new Text(theme.fg("dim", "↑↓ navigate • enter select • esc close"), 1, 0),
						);
						container.addChild(new DynamicBorder((s) => theme.fg("accent", s)));

						return {
							render: (w) => container.render(w),
							invalidate: () => container.invalidate(),
							handleInput: (data) => {
								list.handleInput(data);
								tui.requestRender();
							},
						};
					},
					{ overlay: true },
				);

				if (!picked) return;
				if (picked === "__accept__") {
					await acceptAll(ctx);
					return;
				}
				if (picked === "__decline__") {
					await declineAll(ctx);
					return;
				}

				const t = tracked.get(picked);
				if (!t) {
					ctx.ui.notify("filechanges: entry not found (maybe log was cleared).", "warning");
					continue;
				}

				const md = `\`\`\`diff\n${t.diff.trimEnd() || "(no diff)"}\n\`\`\``;
				await ctx.ui.custom<void>(
					(tui, theme, _kb, done) => {
						const container = new Container();
						container.addChild(new DynamicBorder((s) => theme.fg("accent", s)));
						container.addChild(new Text(theme.fg("accent", theme.bold(t.displayPath)), 1, 0));
						container.addChild(new Markdown(md, 1, 0, getMarkdownTheme()));
						container.addChild(new Text(theme.fg("dim", "esc to go back"), 1, 0));
						container.addChild(new DynamicBorder((s) => theme.fg("accent", s)));

						return {
							render: (w) => container.render(w),
							invalidate: () => container.invalidate(),
							handleInput: (data) => {
								if (matchesKey(data, Key.escape) || matchesKey(data, Key.ctrl("c"))) done();
								else tui.requestRender();
							},
						};
					},
					{ overlay: true },
				);
			}
		},
	});

	pi.registerCommand("filechanges-accept", {
		description: "Accept pi-made changes (keeps files, clears log)",
		handler: async (args, ctx: ExtensionCommandContext) => {
			(ctx as any).args = args ? args.split(/\s+/g).filter(Boolean) : [];
			await acceptAll(ctx);
		},
	});

	pi.registerCommand("filechanges-decline", {
		description: "Decline pi-made changes (reverts files, clears log)",
		handler: async (args, ctx: ExtensionCommandContext) => {
			(ctx as any).args = args ? args.split(/\s+/g).filter(Boolean) : [];
			await declineAll(ctx);
		},
	});

	// ── Session lifecycle ──

	pi.on("session_start", async (_event, ctx) => {
		await rebuildFromSession(ctx);
	});

	pi.on("session_switch", async (_event, ctx) => {
		await rebuildFromSession(ctx);
	});

	pi.on("session_fork", async (_event, ctx) => {
		await rebuildFromSession(ctx);
	});

	// ── Capture before snapshots ──

	pi.on("tool_call", async (event, ctx) => {
		if (isToolCallEventType("edit", event) || isToolCallEventType("write", event)) {
			const { absPath, relPath } = normalizeToolPath(ctx.cwd, event.input.path);
			const before = await readTextOrNull(absPath);
			pendingByToolCallId.set(event.toolCallId, { path: relPath, absPath, before });
		}
	});

	// ── Commit on success ──

	pi.on("tool_result", async (event, ctx) => {
		if (event.isError) {
			pendingByToolCallId.delete(event.toolCallId);
			return;
		}

		if (!isEditToolResult(event) && !isWriteToolResult(event)) return;

		const pending = pendingByToolCallId.get(event.toolCallId);
		pendingByToolCallId.delete(event.toolCallId);
		if (!pending) return;

		if (!baselines.has(pending.path)) {
			baselines.set(pending.path, {
				path: pending.path,
				absPath: pending.absPath,
				originalContent: pending.before,
				createdAt: Date.now(),
			});
			pi.appendEntry(ENTRY_BASELINE, {
				path: pending.path,
				originalContent: pending.before,
				timestamp: Date.now(),
			});
		}

		await recomputeTrackedFile(ctx, pending.path);

		// Untrack if back to baseline
		const baseline = baselines.get(pending.path);
		const current = await readTextOrNull(pending.absPath);
		if (baseline) {
			const back =
				(baseline.originalContent !== null && current === baseline.originalContent) ||
				(baseline.originalContent === null && current === null);
			if (back) {
				baselines.delete(pending.path);
				tracked.delete(pending.path);
				pi.appendEntry(ENTRY_UNTRACK, { path: pending.path, timestamp: Date.now() });
			}
		}

		updateUi(ctx);
	});
}
