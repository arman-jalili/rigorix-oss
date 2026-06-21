/**
 * Skill Curator Extension for pi
 *
 * Background maintenance for agent-created skills.
 * Tracks usage (view, use, patch counts), detects stale/unused skills,
 * and recommends consolidation or archival.
 *
 * Inspired by Hermes-Agent Curator — adapted for Guardian's skill system
 * where skills live in .pi/skills/ and are loaded via snippet expansion.
 */

import {
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	renameSync,
	statSync,
	writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";

// ── Types ──

type ExtensionContext = {
	cwd: string;
	ui: {
		notify(message: string, level?: string): void;
		setStatus(key: string, message: string | null): void;
		confirm(title: string, message: string): Promise<boolean>;
		select(title: string, options: string[]): Promise<string | null>;
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

type SkillUsage = {
	name: string;
	useCount: number;
	viewCount: number;
	patchCount: number;
	lastUsedAt: string | null;
	lastViewedAt: string | null;
	lastPatchedAt: string | null;
	createdAt: string;
	state: "active" | "stale" | "archived";
	pinned: boolean;
};

type CuratorState = {
	skills: Record<string, SkillUsage>;
	lastRunAt: string | null;
	runCount: number;
};

// ── Constants ──

const USAGE_FILE = ".pi/.guardian-skill-usage.json";
const ARCHIVE_DIR = ".pi/skills/.archive";
const BUNDLED_SKILLS = new Set([
	"architecture-coordinator",
	"architecture-validator",
	"ci-mr-validator",
	"code-developer",
	"commit",
	"debug",
	"documentation-maintainer",
	"integration-validator",
	"issue-creator",
	"land",
	"operations-validator",
	"plan-mode",
	"pull",
	"push",
	"security-validator",
	"session-persistence",
	"slash-commands",
	"snippets",
	"subagent-registry",
	"test-validator",
	// validators
	"architecture-validator",
	"ci-validator",
	"context-compaction",
	"integration-validator",
	"model-registry",
	"operations-validator",
	"security-guards",
	"security-validator",
	"system-prompt-tiers",
	"test-validator",
]);

const STALE_AFTER_DAYS = 30;
const ARCHIVE_AFTER_DAYS = 90;

// ── Persistence ──

function loadUsage(cwd: string): CuratorState {
	const p = join(cwd, USAGE_FILE);
	if (!existsSync(p)) {
		return { skills: {}, lastRunAt: null, runCount: 0 };
	}
	return JSON.parse(readFileSync(p, "utf-8")) as CuratorState;
}

function saveUsage(cwd: string, state: CuratorState): void {
	const p = join(cwd, USAGE_FILE);
	const dir = dirname(p);
	if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
	writeFileSync(p, JSON.stringify(state, null, 2));
}

function ensureSkillTracked(cwd: string, name: string): SkillUsage {
	const state = loadUsage(cwd);
	if (state.skills[name]) return state.skills[name];

	const skillPath = join(cwd, `.pi/skills/agents/${name}.md`);
	const validatorPath = join(cwd, `.pi/skills/validators/${name}.md`);
	const path = existsSync(skillPath) ? skillPath : validatorPath;
	let createdAt = new Date().toISOString();
	if (existsSync(path)) {
		try {
			createdAt = statSync(path).birthtime.toISOString();
		} catch {
			// birthtime not available, use mtime
			createdAt = statSync(path).mtime.toISOString();
		}
	}

	state.skills[name] = {
		name,
		useCount: 0,
		viewCount: 0,
		patchCount: 0,
		lastUsedAt: null,
		lastViewedAt: null,
		lastPatchedAt: null,
		createdAt,
		state: "active",
		pinned: false,
	};
	saveUsage(cwd, state);
	return state.skills[name];
}

// ── Curator ──

class Curator {
	private state: CuratorState;

	constructor(private cwd: string) {
		this.state = loadUsage(cwd);
	}

	recordUse(name: string): void {
		const skill = ensureSkillTracked(this.cwd, name);
		skill.useCount++;
		skill.lastUsedAt = new Date().toISOString();
		if (skill.state === "stale") skill.state = "active";
		saveUsage(this.cwd, this.state);
	}

	recordView(name: string): void {
		const skill = ensureSkillTracked(this.cwd, name);
		skill.viewCount++;
		skill.lastViewedAt = new Date().toISOString();
		saveUsage(this.cwd, this.state);
	}

	recordPatch(name: string): void {
		const skill = ensureSkillTracked(this.cwd, name);
		skill.patchCount++;
		skill.lastPatchedAt = new Date().toISOString();
		saveUsage(this.cwd, this.state);
	}

	/**
	 * Run the curator review pass.
	 * Returns a report of actions taken.
	 */
	review(dryRun = false): { actions: string[]; summary: string } {
		const actions: string[] = [];
		const now = Date.now();
		const staleThreshold = STALE_AFTER_DAYS * 24 * 60 * 60 * 1000;
		const archiveThreshold = ARCHIVE_AFTER_DAYS * 24 * 60 * 60 * 1000;

		// Discover all skills on disk
		const skillDirs = ["agents", "validators"];
		const diskSkills = new Set<string>();
		for (const dir of skillDirs) {
			const skillPath = join(this.cwd, `.pi/skills/${dir}`);
			if (existsSync(skillPath)) {
				try {
					for (const f of readdirSync(skillPath)) {
						if (f.endsWith(".md")) {
							diskSkills.add(f.replace(".md", ""));
						}
					}
				} catch {
					// ignore
				}
			}
		}

		// Transition skills based on usage age
		for (const [name, skill] of Object.entries(this.state.skills)) {
			// Skip bundled skills
			if (BUNDLED_SKILLS.has(name)) continue;
			// Skip pinned skills
			if (skill.pinned) continue;
			// Skip already archived
			if (skill.state === "archived") continue;

			const lastActivity = skill.lastUsedAt
				? new Date(skill.lastUsedAt).getTime()
				: new Date(skill.createdAt).getTime();
			const age = now - lastActivity;

			if (age > archiveThreshold && skill.state !== "archived") {
				if (!dryRun) {
					this.archiveSkill(name);
				}
				actions.push(
					`📦 archived ${name} (unused for ${Math.round(age / (24 * 60 * 60 * 1000))} days)`,
				);
				skill.state = "archived";
			} else if (age > staleThreshold && skill.state === "active") {
				skill.state = "stale";
				actions.push(
					`⚠️ marked stale: ${name} (unused for ${Math.round(age / (24 * 60 * 60 * 1000))} days)`,
				);
			}
		}

		// Detect skills on disk but not tracked
		for (const name of diskSkills) {
			if (!BUNDLED_SKILLS.has(name) && !this.state.skills[name]) {
				ensureSkillTracked(this.cwd, name);
			}
		}

		// Update metadata
		this.state.lastRunAt = new Date().toISOString();
		this.state.runCount++;
		if (!dryRun) saveUsage(this.cwd, this.state);

		// Build summary
		const totalSkills = Object.keys(this.state.skills).length;
		const activeCount = Object.values(this.state.skills).filter((s) => s.state === "active").length;
		const staleCount = Object.values(this.state.skills).filter((s) => s.state === "stale").length;
		const archivedCount = Object.values(this.state.skills).filter(
			(s) => s.state === "archived",
		).length;
		const pinnedCount = Object.values(this.state.skills).filter((s) => s.pinned).length;

		const summary = `Curator Review #${this.state.runCount}\nTotal: ${totalSkills} | Active: ${activeCount} | Stale: ${staleCount} | Archived: ${archivedCount} | Pinned: ${pinnedCount}\n${actions.length > 0 ? `\nActions:\n${actions.join("\n")}` : "\nNo actions needed."}`;

		return { actions, summary };
	}

	private archiveSkill(name: string): void {
		const dirs = ["agents", "validators"];
		for (const dir of dirs) {
			const src = join(this.cwd, `.pi/skills/${dir}/${name}.md`);
			if (existsSync(src)) {
				const archivePath = join(this.cwd, ARCHIVE_DIR);
				if (!existsSync(archivePath)) mkdirSync(archivePath, { recursive: true });
				const dest = join(archivePath, `${name}.md`);
				renameSync(src, dest);
				break;
			}
		}
	}

	pin(name: string): boolean {
		const skill = this.state.skills[name];
		if (!skill) return false;
		skill.pinned = true;
		saveUsage(this.cwd, this.state);
		return true;
	}

	unpin(name: string): boolean {
		const skill = this.state.skills[name];
		if (!skill) return false;
		skill.pinned = false;
		saveUsage(this.cwd, this.state);
		return true;
	}

	restore(name: string): boolean {
		const skill = this.state.skills[name];
		if (!skill) return false;
		skill.state = "active";
		saveUsage(this.cwd, this.state);

		// Move from archive back
		const archivePath = join(this.cwd, ARCHIVE_DIR, `${name}.md`);
		if (existsSync(archivePath)) {
			const dest = join(this.cwd, `.pi/skills/agents/${name}.md`);
			renameSync(archivePath, dest);
		}
		return true;
	}

	statusReport(): string {
		const skills = Object.values(this.state.skills);
		const active = skills
			.filter((s) => s.state === "active")
			.sort((a, b) => b.useCount - a.useCount);
		const stale = skills.filter((s) => s.state === "stale");
		const archived = skills.filter((s) => s.state === "archived");
		const pinned = skills.filter((s) => s.pinned);

		const lines = [
			"## Guardian Skill Curator Status",
			"",
			`**Last review:** ${this.state.lastRunAt || "never"}`,
			`**Reviews run:** ${this.state.runCount}`,
			"",
			`**Active:** ${active.length} | **Stale:** ${stale.length} | **Archived:** ${archived.length} | **Pinned:** ${pinned.length}`,
		];

		if (active.length > 0) {
			lines.push("\n### Most Used Skills");
			for (const s of active.slice(0, 10)) {
				lines.push(
					`  - ${s.name}: ${s.useCount} uses, ${s.viewCount} views, ${s.patchCount} patches`,
				);
			}
		}

		if (stale.length > 0) {
			lines.push("\n### Stale Skills (will archive if unused)");
			for (const s of stale) {
				lines.push(`  - ${s.name} (last used: ${s.lastUsedAt || "never"})`);
			}
		}

		if (pinned.length > 0) {
			lines.push("\n### Pinned Skills (protected from archival)");
			for (const s of pinned) {
				lines.push(`  - ${s.name}`);
			}
		}

		if (archived.length > 0) {
			lines.push("\n### Archived Skills");
			for (const s of archived) {
				lines.push(`  - ${s.name} (restore with /curator restore ${s.name})`);
			}
		}

		return lines.join("\n");
	}
}

// ── Helpers ──

function toolResult(text: string) {
	return { content: [{ type: "text" as const, text }] };
}

function toolError(text: string) {
	return { content: [{ type: "text" as const, text }], isError: true };
}

// ── Extension ──

export default function (pi: ExtensionAPI) {
	let curator: Curator | null = null;

	pi.on("session_start", async (_event, ctx) => {
		curator = new Curator(ctx.cwd);
		ctx.ui.notify("Guardian skill curator initialized", "info");
	});

	// ── curator_review tool ──
	pi.registerTool({
		name: "curator_review",
		label: "Skill Curator Review",
		description:
			"Run the skill curator review pass. Detects stale/unused skills and recommends archival.",
		parameters: {
			type: "object",
			properties: {
				dryRun: { type: "boolean", description: "Preview only, no mutations" },
			},
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!curator) curator = new Curator(ctx.cwd);
			const dryRun = (params.dryRun as boolean) || false;
			const result = curator.review(dryRun);
			return toolResult(result.summary);
		},
	});

	// ── curator_pin tool ──
	pi.registerTool({
		name: "curator_pin",
		label: "Pin Skill",
		description: "Pin a skill to protect it from curator archival.",
		parameters: {
			type: "object",
			properties: {
				name: { type: "string", description: "Skill name" },
			},
			required: ["name"],
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!curator) curator = new Curator(ctx.cwd);
			const name = (params.name as string)?.trim();
			if (!name) return toolError("name is required");
			if (curator.pin(name)) {
				return toolResult(`📌 Pinned: ${name}`);
			}
			return toolError(`Skill ${name} not found`);
		},
	});

	// ── curator_unpin tool ──
	pi.registerTool({
		name: "curator_unpin",
		label: "Unpin Skill",
		description: "Unpin a skill to allow curator archival.",
		parameters: {
			type: "object",
			properties: {
				name: { type: "string", description: "Skill name" },
			},
			required: ["name"],
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!curator) curator = new Curator(ctx.cwd);
			const name = (params.name as string)?.trim();
			if (!name) return toolError("name is required");
			if (curator.unpin(name)) {
				return toolResult(`Unpinned: ${name}`);
			}
			return toolError(`Skill ${name} not found`);
		},
	});

	// ── /curator command ──
	pi.registerCommand("curator", {
		description: "Manage Guardian skill curator",
		handler: async (args, ctx) => {
			if (!curator) curator = new Curator(ctx.cwd);
			const raw = typeof args === "string" ? args : "";
			const tokens = raw.split(/\s+/).filter(Boolean);
			const action = tokens[0];

			if (!action || action === "status") {
				ctx.ui.notify(curator.statusReport(), "info");
				return;
			}

			if (action === "review") {
				const dryRun = tokens.includes("--dry-run");
				const result = curator.review(dryRun);
				ctx.ui.notify(result.summary, dryRun ? "info" : "success");
				return;
			}

			if (action === "pin") {
				const name = tokens[1];
				if (!name) {
					ctx.ui.notify("Usage: /curator pin <skill-name>", "error");
					return;
				}
				if (curator.pin(name)) {
					ctx.ui.notify(`📌 Pinned: ${name}`, "success");
				} else {
					ctx.ui.notify(`Skill ${name} not found`, "error");
				}
				return;
			}

			if (action === "unpin") {
				const name = tokens[1];
				if (!name) {
					ctx.ui.notify("Usage: /curator unpin <skill-name>", "error");
					return;
				}
				if (curator.unpin(name)) {
					ctx.ui.notify(`Unpinned: ${name}`, "info");
				} else {
					ctx.ui.notify(`Skill ${name} not found`, "error");
				}
				return;
			}

			if (action === "restore") {
				const name = tokens[1];
				if (!name) {
					ctx.ui.notify("Usage: /curator restore <skill-name>", "error");
					return;
				}
				if (curator.restore(name)) {
					ctx.ui.notify(`Restored: ${name}`, "success");
				} else {
					ctx.ui.notify(`Skill ${name} not found`, "error");
				}
				return;
			}

			ctx.ui.notify("Usage: /curator [status|review [--dry-run]|pin|unpin|restore] <name>", "info");
		},
	});
}
