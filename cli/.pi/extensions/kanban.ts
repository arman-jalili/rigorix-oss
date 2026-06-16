/**
 * Kanban Extension for pi
 *
 * Durable JSON-backed task board for multi-agent collaboration.
 * Tasks have states (triage → todo → ready → running → blocked → done → archived),
 * dependency links, comments, and workspace management.
 *
 * Inspired by Hermes-Agent Kanban — adapted for Guardian's validation-first model
 * where task state transitions can trigger validator runs.
 */

import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
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

type TaskStatus = "triage" | "todo" | "ready" | "running" | "blocked" | "done" | "archived";

type Task = {
	id: string;
	title: string;
	body: string;
	assignee: string;
	status: TaskStatus;
	blockReason: string | null;
	priority: "low" | "medium" | "high" | "critical";
	parents: string[]; // dependency links
	workspace: string; // "scratch" | "dir:/path" | "worktree"
	createdAt: string;
	updatedAt: string;
	claimedAt: string | null;
	comments: Comment[];
};

type Comment = {
	id: string;
	author: string;
	text: string;
	createdAt: string;
};

type KanbanBoard = {
	tasks: Task[];
	nextId: number;
};

// ── Persistence ──

const KANBAN_FILE = ".pi/.guardian-kanban.json";

function loadBoard(cwd: string): KanbanBoard {
	const p = join(cwd, KANBAN_FILE);
	if (!existsSync(p)) {
		const board: KanbanBoard = { tasks: [], nextId: 1 };
		saveBoard(cwd, board);
		return board;
	}
	return JSON.parse(readFileSync(p, "utf-8")) as KanbanBoard;
}

function saveBoard(cwd: string, board: KanbanBoard): void {
	const p = join(cwd, KANBAN_FILE);
	const dir = dirname(p);
	if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
	writeFileSync(p, JSON.stringify(board, null, 2));
}

function generateId(board: KanbanBoard): string {
	const id = `TK-${String(board.nextId).padStart(4, "0")}`;
	board.nextId++;
	return id;
}

function findTask(board: KanbanBoard, id: string): Task | undefined {
	return board.tasks.find((t) => t.id === id);
}

// ── KanbanManager ──

class KanbanManager {
	private board: KanbanBoard;

	constructor(private cwd: string) {
		this.board = loadBoard(cwd);
	}

	createTask(
		title: string,
		opts?: {
			body?: string;
			assignee?: string;
			priority?: Task["priority"];
			parents?: string[];
			workspace?: string;
		},
	): Task {
		const task: Task = {
			id: generateId(this.board),
			title,
			body: opts?.body || "",
			assignee: opts?.assignee || "unassigned",
			status: "todo",
			blockReason: null,
			priority: opts?.priority || "medium",
			parents: opts?.parents || [],
			workspace: opts?.workspace || "scratch",
			createdAt: new Date().toISOString(),
			updatedAt: new Date().toISOString(),
			claimedAt: null,
			comments: [],
		};
		this.board.tasks.push(task);
		this.save();
		return task;
	}

	getTask(id: string): Task | undefined {
		return findTask(this.board, id);
	}

	listTasks(filter?: { status?: TaskStatus; assignee?: string }): Task[] {
		let tasks = [...this.board.tasks];
		if (filter?.status) tasks = tasks.filter((t) => t.status === filter.status);
		if (filter?.assignee) tasks = tasks.filter((t) => t.assignee === filter.assignee);
		// Sort by priority then creation date
		const priorityOrder = { critical: 0, high: 1, medium: 2, low: 3 } as const;
		tasks.sort(
			(a, b) => priorityOrder[a.priority] - priorityOrder[b.priority] || a.id.localeCompare(b.id),
		);
		return tasks;
	}

	updateStatus(id: string, status: TaskStatus, reason?: string): Task | undefined {
		const task = findTask(this.board, id);
		if (!task) return undefined;

		// Validate transitions
		const validTransitions: Record<TaskStatus, TaskStatus[]> = {
			triage: ["todo", "blocked", "archived"],
			todo: ["ready", "running", "blocked", "archived"],
			ready: ["running", "blocked", "todo", "archived"],
			running: ["done", "blocked", "todo"],
			blocked: ["todo", "running", "archived"],
			done: ["archived"],
			archived: [],
		};

		if (!validTransitions[task.status].includes(status)) {
			throw new Error(
				`Invalid transition: ${task.status} → ${status}. Allowed: ${validTransitions[task.status].join(", ")}`,
			);
		}

		task.status = status;
		task.updatedAt = new Date().toISOString();

		if (status === "blocked" && reason) {
			task.blockReason = reason;
		}
		if (status === "running") {
			task.claimedAt = new Date().toISOString();
		}
		if (status === "done") {
			// Auto-unblock children
			this.checkReadyChildren(id);
		}

		this.save();
		return task;
	}

	addComment(taskId: string, author: string, text: string): Comment | undefined {
		const task = findTask(this.board, taskId);
		if (!task) return undefined;
		const comment: Comment = {
			id: `C-${Date.now()}`,
			author,
			text,
			createdAt: new Date().toISOString(),
		};
		task.comments.push(comment);
		task.updatedAt = new Date().toISOString();
		this.save();
		return comment;
	}

	block(id: string, reason: string): Task | undefined {
		return this.updateStatus(id, "blocked", reason);
	}

	complete(id: string): Task | undefined {
		return this.updateStatus(id, "done");
	}

	archive(id: string): Task | undefined {
		return this.updateStatus(id, "archived");
	}

	private checkReadyChildren(parentId: string): void {
		for (const task of this.board.tasks) {
			if (task.parents.includes(parentId) && task.status === "todo") {
				const allDone = task.parents.every((pid) => {
					const parent = findTask(this.board, pid);
					return parent?.status === "done";
				});
				if (allDone) {
					task.status = "ready";
					task.updatedAt = new Date().toISOString();
				}
			}
		}
		this.save();
	}

	private save(): void {
		saveBoard(this.cwd, this.board);
	}
}

// ── Helpers ──

function formatTask(t: Task): string {
	const statusEmoji: Record<TaskStatus, string> = {
		triage: "🔍",
		todo: "📋",
		ready: "⚡",
		running: "🔄",
		blocked: "🚧",
		done: "✅",
		archived: "📦",
	};
	const priorityEmoji: Record<string, string> = {
		critical: "🔴",
		high: "🟠",
		medium: "🟡",
		low: "🟢",
	};
	const blockInfo = t.blockReason ? ` [blocked: ${t.blockReason}]` : "";
	const parentInfo = t.parents.length > 0 ? ` (deps: ${t.parents.join(", ")})` : "";
	const commentInfo = t.comments.length > 0 ? ` (${t.comments.length} comments)` : "";
	return `${statusEmoji[t.status]} **${t.id}** ${priorityEmoji[t.priority]} ${t.title} → ${t.assignee}${blockInfo}${parentInfo}${commentInfo}`;
}

function toolResult(text: string) {
	return { content: [{ type: "text" as const, text }] };
}

function toolError(text: string) {
	return { content: [{ type: "text" as const, text }], isError: true };
}

// ── Extension ──

export default function (pi: ExtensionAPI) {
	let manager: KanbanManager | null = null;

	pi.on("session_start", async (_event, ctx) => {
		manager = new KanbanManager(ctx.cwd);
		ctx.ui.notify("Guardian kanban board initialized", "info");
	});

	// ── kanban_create ──
	pi.registerTool({
		name: "kanban_create",
		label: "Kanban Create Task",
		description:
			"Create a new task on the Guardian kanban board for tracking work across sessions.",
		parameters: {
			type: "object",
			properties: {
				title: { type: "string", description: "Task title" },
				body: { type: "string", description: "Task description / body" },
				assignee: { type: "string", description: "Agent or person assigned" },
				priority: {
					type: "string",
					enum: ["low", "medium", "high", "critical"],
					description: "Task priority",
				},
				parents: {
					type: "array",
					items: { type: "string" },
					description: "Parent task IDs (dependencies)",
				},
			},
			required: ["title"],
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new KanbanManager(ctx.cwd);
			const title = (params.title as string)?.trim();
			if (!title) return toolError("title is required");

			try {
				const task = manager.createTask(title, {
					body: (params.body as string) || "",
					assignee: (params.assignee as string) || "agent",
					priority: (params.priority as Task["priority"]) || "medium",
					parents: Array.isArray(params.parents) ? (params.parents as string[]) : undefined,
				});
				return toolResult(`Created task:\n\n${formatTask(task)}`);
			} catch (e) {
				return toolError(`Failed to create task: ${e}`);
			}
		},
	});

	// ── kanban_list ──
	pi.registerTool({
		name: "kanban_list",
		label: "Kanban List Tasks",
		description: "List tasks on the Guardian kanban board, optionally filtered by status.",
		parameters: {
			type: "object",
			properties: {
				status: {
					type: "string",
					enum: ["triage", "todo", "ready", "running", "blocked", "done", "archived", "all"],
					description: "Filter by status (default: all active)",
				},
			},
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new KanbanManager(ctx.cwd);
			const status = (params.status as string) || "all";
			const filter = status === "all" ? undefined : { status: status as TaskStatus };
			const tasks = manager.listTasks(filter);

			if (tasks.length === 0) {
				return toolResult(`No tasks found${status !== "all" ? ` with status=${status}` : ""}.`);
			}

			const lines = [`## Kanban Board — ${tasks.length} task(s)\n`];
			for (const t of tasks) {
				lines.push(formatTask(t));
			}
			return toolResult(lines.join("\n"));
		},
	});

	// ── kanban_show ──
	pi.registerTool({
		name: "kanban_show",
		label: "Kanban Show Task",
		description: "Show full details of a specific kanban task including comments.",
		parameters: {
			type: "object",
			properties: {
				id: { type: "string", description: "Task ID (e.g., TK-0001)" },
			},
			required: ["id"],
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new KanbanManager(ctx.cwd);
			const id = (params.id as string)?.trim();
			if (!id) return toolError("id is required");

			const task = manager.getTask(id);
			if (!task) return toolError(`Task ${id} not found`);

			const lines = [
				`## ${task.id}: ${task.title}`,
				"",
				`**Status:** ${task.status}`,
				`**Priority:** ${task.priority}`,
				`**Assignee:** ${task.assignee}`,
				`**Created:** ${task.createdAt}`,
				`**Updated:** ${task.updatedAt}`,
				task.blockReason ? `**Block Reason:** ${task.blockReason}` : "",
				task.parents.length > 0 ? `**Dependencies:** ${task.parents.join(", ")}` : "",
				"",
				task.body ? `### Description\n\n${task.body}\n` : "",
			].filter(Boolean);

			if (task.comments.length > 0) {
				lines.push("### Comments\n");
				for (const c of task.comments) {
					lines.push(`**${c.author}** (${c.createdAt}): ${c.text}`);
				}
			}

			return toolResult(lines.join("\n"));
		},
	});

	// ── kanban_complete ──
	pi.registerTool({
		name: "kanban_complete",
		label: "Kanban Complete Task",
		description: "Mark a kanban task as done.",
		parameters: {
			type: "object",
			properties: {
				id: { type: "string", description: "Task ID" },
			},
			required: ["id"],
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new KanbanManager(ctx.cwd);
			const id = (params.id as string)?.trim();
			if (!id) return toolError("id is required");

			const task = manager.complete(id);
			if (!task) return toolError(`Task ${id} not found`);
			return toolResult(`✅ Completed: ${task.id} — ${task.title}`);
		},
	});

	// ── kanban_block ──
	pi.registerTool({
		name: "kanban_block",
		label: "Kanban Block Task",
		description: "Block a kanban task with a reason.",
		parameters: {
			type: "object",
			properties: {
				id: { type: "string", description: "Task ID" },
				reason: { type: "string", description: "Why the task is blocked" },
			},
			required: ["id", "reason"],
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new KanbanManager(ctx.cwd);
			const id = (params.id as string)?.trim();
			const reason = (params.reason as string)?.trim();
			if (!id || !reason) return toolError("id and reason are required");

			const task = manager.block(id, reason);
			if (!task) return toolError(`Task ${id} not found`);
			return toolResult(`🚧 Blocked: ${task.id} — ${task.title} (${reason})`);
		},
	});

	// ── kanban_comment ──
	pi.registerTool({
		name: "kanban_comment",
		label: "Kanban Add Comment",
		description: "Add a comment to a kanban task.",
		parameters: {
			type: "object",
			properties: {
				id: { type: "string", description: "Task ID" },
				text: { type: "string", description: "Comment text" },
			},
			required: ["id", "text"],
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new KanbanManager(ctx.cwd);
			const id = (params.id as string)?.trim();
			const text = (params.text as string)?.trim();
			if (!id || !text) return toolError("id and text are required");

			const comment = manager.addComment(id, "agent", text);
			if (!comment) return toolError(`Task ${id} not found`);
			return toolResult(`💬 Comment added to ${id}`);
		},
	});

	// ── /kanban command ──
	pi.registerCommand("kanban", {
		description: "Manage the Guardian kanban board",
		handler: async (args, ctx) => {
			if (!manager) manager = new KanbanManager(ctx.cwd);
			const raw = typeof args === "string" ? args : "";
			const tokens = raw.split(/\s+/).filter(Boolean);
			const action = tokens[0];

			if (!action || action === "status") {
				const tasks = manager.listTasks();
				const byStatus: Record<string, number> = {};
				for (const t of tasks) {
					byStatus[t.status] = (byStatus[t.status] || 0) + 1;
				}
				const summary = Object.entries(byStatus)
					.map(([s, c]) => `${s}: ${c}`)
					.join(" | ");
				ctx.ui.notify(`Kanban: ${tasks.length} total — ${summary}`, "info");
				return;
			}

			if (action === "create") {
				const title = tokens.slice(1).join(" ");
				if (!title) {
					ctx.ui.notify("Usage: /kanban create <title>", "error");
					return;
				}
				const task = manager.createTask(title);
				ctx.ui.notify(`Created: ${task.id} — ${task.title}`, "success");
				return;
			}

			if (action === "list") {
				const status = tokens[1] as TaskStatus | undefined;
				const tasks = manager.listTasks(status ? { status } : undefined);
				if (tasks.length === 0) {
					ctx.ui.notify("No tasks found", "info");
					return;
				}
				const lines = tasks.map(formatTask);
				ctx.ui.notify(lines.join("\n"), "info");
				return;
			}

			ctx.ui.notify("Usage: /kanban [status|create <title>|list [status]]", "info");
		},
	});
}
