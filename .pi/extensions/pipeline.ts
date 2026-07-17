/**
 * Canonical Reference: .pi/architecture/modules/core-libraries.md
 * Last Sync: 2026-05-31

 * Pipeline Extension for pi
 *
 * Multi-step workflow engine that iterates over items (issues, tasks, etc.)
 * with per-step prompts and acceptance conditions.
 *
 * Example: "Close all P1 bugs" with steps [implement, validate, create-mr, merge]
 * Each step has its own acceptance gate (validator, shell, LLM, or none).
 *
 * Commands:
 *   /pipeline <name> --items "id1,id2" --steps "implement,validate,create-mr"
 *   /pipeline status              Show current pipeline progress
 *   /pipeline pause               Pause at current step
 *   /pipeline resume              Resume from where paused
 *   /pipeline skip-step           Skip current step
 *   /pipeline retry-step          Retry current step
 *   /pipeline abort               Kill pipeline
 */

import { execFileSync, execSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import {
	fetchIssueContent,
	forgePrExists as adapterForgePrExists,
	forgePrMerged as adapterForgePrMerged,
	readRepoTool,
	readRepository,
	getGitBaseUrl,
	runScript,
} from "./architect-lib/forge-adapter.ts";

// ── Validator Scripts ──

const VALIDATOR_SCRIPTS: Record<string, string> = {
	ci: ".pi/scripts/validate-ci.sh",
	tests: ".pi/scripts/validate-tests.sh",
	security: ".pi/scripts/validate-security.sh",
	operations: ".pi/scripts/validate-operations.sh",
	architecture: ".pi/scripts/validate-architecture.sh",
	canonical: ".pi/scripts/validate-canonical.sh",
	integration: ".pi/scripts/validate-integration.sh",
};

// ── Helpers ──

/**
 * Parse CLI-style arguments with quote awareness.
 * Handles double-quoted strings so that `--name "My Epic"` yields ["--name", "My Epic"].
 */
function parseArgs(raw: string): string[] {
	const tokens: string[] = [];
	let current = "";
	let inQuote = false;
	for (const ch of raw) {
		if (ch === '"') {
			inQuote = !inQuote;
			continue;
		}
		if (!inQuote && ch === " ") {
			if (current) { tokens.push(current); current = ""; }
			continue;
		}
		current += ch;
	}
	if (current) tokens.push(current);
	return tokens;
}

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

// ── Pipeline Schema ──

type StepName = string;

type StepConfig = {
	name: string;
	prompt?: string; // .pi/prompts/ path
	acceptance: AcceptanceConfig;
};

type AcceptanceConfig =
	| { type: "validator"; validators: string[] }
	| { type: "shell"; command: string }
	| { type: "llm"; prompt: string }
	| { type: "none" };

type PipelineStatus = "running" | "paused" | "done" | "failed" | "aborted";

type ItemResult = {
	item: string;
	status: "done" | "failed" | "skipped" | "in-progress";
	stepResults: StepResult[];
};

type StepResult = {
	step: string;
	status: "passed" | "failed" | "skipped";
	reason: string;
};

type PipelineState = {
	id: string;
	name: string;
	items: string[];
	steps: StepConfig[];
	currentItemIndex: number;
	currentStepIndex: number;
	status: PipelineStatus;
	retryCount: number;
	results: ItemResult[];
	mergeOnValid: boolean;
	createdAt: string;
	updatedAt: string;
};

// ── Constants ──

const PIPELINE_STATE_KEY = ".pi/.guardian-pipeline-state.json";

// ── Persistence ──

function loadPipelineState(cwd: string): PipelineState | null {
	const p = join(cwd, PIPELINE_STATE_KEY);
	if (!existsSync(p)) return null;
	try {
		return JSON.parse(readFileSync(p, "utf-8")) as PipelineState;
	} catch {
		return null;
	}
}

function savePipelineState(cwd: string, state: PipelineState): void {
	const p = join(cwd, PIPELINE_STATE_KEY);
	const dir = dirname(p);
	if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
	writeFileSync(p, JSON.stringify(state, null, 2));
}

// ── Helpers ──

function generatePipelineId(): string {
	return `PL-${String(Math.floor(Math.random() * 10000)).padStart(4, "0")}`;
}

function formatPipelineProgress(state: PipelineState): string {
	const total = state.items.length * state.steps.length;
	const completed = state.results.filter((r) => r.status === "done").length;
	const lines = [
		`## Pipeline: ${state.name}`,
		`**Status:** ${state.status}`,
		`**Progress:** ${completed}/${state.items.length} items, ${total === 0 ? 0 : Math.round((completed / total) * 100)}%`,
		"",
	];

	if (state.status === "running" || state.status === "paused") {
		lines.push(
			`**Current item:** ${state.items[state.currentItemIndex]}`,
			`**Current step:** ${state.steps[state.currentStepIndex]?.name}`,
			`**Step:** ${state.currentStepIndex + 1}/${state.steps.length}`,
			`**Item:** ${state.currentItemIndex + 1}/${state.items.length}`,
		);
	}

	if (state.results.length > 0) {
		lines.push("\n### Results");
		for (const r of state.results) {
			const emoji = r.status === "done" ? "✓" : r.status === "failed" ? "✗" : "○";
			lines.push(`  ${emoji} ${r.item} — ${r.status}`);
		}
	}

	return lines.join("\n");
}

function statusLine(state: PipelineState | null): string {
	if (!state) return "No active pipeline. Start one with /pipeline <name> ...";
	const emoji =
		state.status === "running"
			? "▶"
			: state.status === "paused"
				? "⏸"
				: state.status === "done"
					? "✓"
					: "✗";
	return `${emoji} Pipeline "${state.name}" (${state.status}) — ${state.currentItemIndex + 1}/${state.items.length} items`;
}

// ── Pipeline Manager ──

class PipelineManager {
	private state: PipelineState | null;
	private repoTool: string;

	constructor(private cwd: string) {
		this.state = loadPipelineState(cwd);
		this.repoTool = readRepoTool(cwd);
	}

	/**
	 * Reload pipeline state from disk, discarding any cached in-memory state.
	 * Use this when another extension (e.g., architect) may have written state directly.
	 */
	private reloadFromDisk(): void {
		this.state = loadPipelineState(this.cwd);
	}

	/**
	 * Reconcile pipeline state against ground truth (git, GitHub, validators).
	 * Also updates module markdown docs when items complete.
	 * Call this on session_start and before any state-dependent operation.
	 */
	reconcile(): void {
		if (!this.state) return;
		let changed = false;

		for (let i = 0; i < this.state.items.length; i++) {
			const item = this.state.items[i];
			const slug = item.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
			const branch = `feat/${slug}`;
			const result = this.state.results.find((r) => r.item === item);

			// Check ground truth
			const branchExists = this.gitBranchExists(branch);
			const prMerged = this.forgePrMerged(branch);
			const prExists = this.forgePrExists(branch);
			const hasCommits = branchExists && this.branchHasCommits(branch);

			if (prMerged) {
				// All steps done — mark item complete
				if (!result || result.status !== "done") {
					const itemResult: ItemResult = {
						item,
						status: "done",
						stepResults: this.state.steps.map((s) => ({ step: s.name, status: "passed", reason: "reconciled" })),
					};
					// Replace or add
					const idx = this.state.results.findIndex((r) => r.item === item);
					if (idx >= 0) this.state.results[idx] = itemResult;
					else this.state.results.push(itemResult);
					changed = true;
					this.updateModuleDocStatus(item, "implemented");
				}
			} else if (prExists) {
				// PR open — implement+validate+create-mr done, waiting on merge
				if (!result || result.status === "skipped") {
					const steps = this.state.steps;
					const stepResults = steps.map((s, idx) => ({
						step: s.name,
						status: idx < steps.length - 1 ? "passed" as const : "skipped" as const,
						reason: idx < steps.length - 1 ? "reconciled" : "waiting for merge",
					}));
					const itemResult: ItemResult = { item, status: "in-progress", stepResults };
					const idx = this.state.results.findIndex((r) => r.item === item);
					if (idx >= 0) this.state.results[idx] = itemResult;
					else this.state.results.push(itemResult);
					changed = true;
				}
			} else if (hasCommits) {
				// Branch with commits — implement done, rest pending
				if (!result || result.status === "skipped") {
					const stepResults = [{ step: "implement", status: "passed" as const, reason: "reconciled" }];
					const itemResult: ItemResult = { item, status: "in-progress", stepResults };
					const idx = this.state.results.findIndex((r) => r.item === item);
					if (idx >= 0) this.state.results[idx] = itemResult;
					else this.state.results.push(itemResult);
					changed = true;
				}
			}
		}

		// If all items done, update overall status
		if (this.state.results.length === this.state.items.length && this.state.results.length > 0 && this.state.results.every((r) => r.status === "done")) {
			this.state.status = "done";
			this.state.currentItemIndex = this.state.items.length;
			this.state.currentStepIndex = 0;
			this.syncRoadmapState();
			changed = true;
		}

		// Reposition currentItemIndex to first non-done item
		const firstUndone = this.state.items.findIndex((item, idx) => {
			const r = this.state!.results.find((res) => res.item === item);
			return !r || r.status !== "done";
		});
		if (firstUndone >= 0) {
			this.state.currentItemIndex = firstUndone;
			this.state.currentStepIndex = 0;
		} else if (this.state.items.length > 0) {
			// All done
			this.state.currentItemIndex = this.state.items.length;
			this.state.currentStepIndex = 0;
		}

		if (changed) {
			this.state.updatedAt = new Date().toISOString();
			savePipelineState(this.cwd, this.state);
		}
	}

	private gitBranchExists(branch: string): boolean {
		try {
			const result = execFileSync("git", ["branch", "-a", "--list", branch], { cwd: this.cwd, encoding: "utf-8", stdio: ["ignore", "pipe", "ignore"] });
			return result.trim().length > 0;
		} catch { return false; }
	}

	private branchHasCommits(branch: string): boolean {
		try {
			const result = execFileSync("git", ["log", "--oneline", branch], { cwd: this.cwd, encoding: "utf-8", stdio: ["ignore", "pipe", "ignore"] });
			return result.trim().length > 0;
		} catch { return false; }
	}

	/**
	 * Check if a PR/MR exists for the given branch, dispatching by repoTool.
	 */
	private forgePrExists(branch: string): boolean {
		return adapterForgePrExists(this.cwd, branch);
	}

	/**
	 * Check if a PR/MR for the given branch is merged, dispatching by repoTool.
	 */
	private forgePrMerged(branch: string): boolean {
		return adapterForgePrMerged(this.cwd, branch);
	}

	/**
	 * Update module markdown doc status when an item is confirmed done.
	 * Looks for .pi/architecture/modules/<item>.md and replaces `status: planned` with `status: implemented`.
	 */
	private updateModuleDocStatus(item: string, newStatus: string): void {
		const modulesDir = join(this.cwd, ".pi", "architecture", "modules");
		try {
			const files = readdirSync(modulesDir).filter((f) => f.endsWith(".md") && f !== "module-template.md");
			// Match by module name (kebab-case from item name)
			const slug = item.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
			for (const file of files) {
				if (file.replace(".md", "") === slug || file.includes(slug)) {
					const filePath = join(modulesDir, file);
					let content = readFileSync(filePath, "utf-8");
					const oldStatus = content.match(/\*\*Status:\*\*\s*(\w+)/);
					if (oldStatus && oldStatus[1] !== newStatus) {
						content = content.replace(/\*\*Status:\*\*\s*\w+/, `**Status:** ${newStatus}`);
						// Also update status: planned → status: implemented markers
						content = content.replace(/^status: planned$/gm, `status: ${newStatus}`);
						writeFileSync(filePath, content, "utf-8");
					}
					break;
				}
			}
		} catch { /* modules dir may not exist */ }
	}

	/**
	 * Verify the current step's work actually happened before advancing.
	 * For "implement" step: check git branch has commits.
	 * For "validate" step: run acceptance gates.
	 * For "create-mr" step: check PR exists.
	 * For "merge" step: check PR is merged.
	 */
	verifyCurrentStep(): { verified: boolean; reason: string } {
		if (!this.state) return { verified: false, reason: "No pipeline state" };
		const item = this.state.items[this.state.currentItemIndex];
		const step = this.state.steps[this.state.currentStepIndex];
		if (!step) return { verified: true, reason: "No current step" };

		const slug = item.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
		const branch = `feat/${slug}`;

		switch (step.name) {
			case "implement": {
				if (!this.gitBranchExists(branch) || !this.branchHasCommits(branch)) {
					return { verified: false, reason: `No commits found on branch ${branch}. Implement the feature first.` };
				}
				return { verified: true, reason: "Commits found on branch" };
			}
			case "create-mr": {
				if (!this.forgePrExists(branch)) {
					return { verified: false, reason: `No PR/MR found for branch ${branch}. Create a PR/MR first.` };
				}
				return { verified: true, reason: "PR/MR exists" };
			}
			case "merge": {
				if (!this.forgePrMerged(branch)) {
					return { verified: false, reason: `PR/MR for ${branch} is not merged yet.` };
				}
				return { verified: true, reason: "PR/MR merged" };
			}
			default:
				return { verified: true, reason: "Unknown step — skipping verification" };
		}
	}

	getState(): PipelineState | null {
		return this.state;
	}

	create(
		name: string,
		items: string[],
		steps: StepConfig[],
		opts: { mergeOnValid?: boolean } = {},
	): PipelineState {
		this.state = {
			id: generatePipelineId(),
			name,
			items,
			steps,
			currentItemIndex: 0,
			currentStepIndex: 0,
			status: "running",
			retryCount: 0,
			results: [],
			mergeOnValid: opts.mergeOnValid ?? false,
			createdAt: new Date().toISOString(),
			updatedAt: new Date().toISOString(),
		};
		savePipelineState(this.cwd, this.state);
		return this.state;
	}

	pause(): void {
		if (!this.state) return;
		this.state.status = "paused";
		this.state.updatedAt = new Date().toISOString();
		savePipelineState(this.cwd, this.state);
	}

	resume(): void {
		if (!this.state) return;
		if (this.state.status === "paused") {
			this.state.status = "running";
			this.state.updatedAt = new Date().toISOString();
			savePipelineState(this.cwd, this.state);
		}
	}

	abort(): void {
		if (!this.state) return;
		this.state.status = "aborted";
		this.state.updatedAt = new Date().toISOString();
		savePipelineState(this.cwd, this.state);
	}

	skipStep(): void {
		if (!this.state) return;
		const item = this.state.items[this.state.currentItemIndex];
		const step = this.state.steps[this.state.currentStepIndex];

		// Mark step as skipped
		const result = this.state.results.find((r) => r.item === item);
		if (result) {
			result.stepResults.push({ step: step.name, status: "skipped", reason: "skipped by user" });
		} else {
			this.state.results.push({
				item,
				status: "in-progress",
				stepResults: [{ step: step.name, status: "skipped", reason: "skipped by user" }],
			});
		}

		// Move to next step
		this.advanceStep();
	}

	retryStep(): void {
		if (!this.state) return;
		this.state.retryCount++;
		this.state.updatedAt = new Date().toISOString();
		savePipelineState(this.cwd, this.state);
	}

	advanceStep(): void {
		if (!this.state) return;

		// Auto-run acceptance gates for validate steps
		if (this.state.currentStepIndex < this.state.steps.length) {
			const step = this.state.steps[this.state.currentStepIndex];
			if (step.name === "validate" && step.acceptance.type !== "none") {
				const { allPassed, errors } = this.runAcceptanceGates(step);
				if (!allPassed) {
					const item = this.state.items[this.state.currentItemIndex];
					this.markStepFailed(step.name, `Acceptance failed: ${errors.join("; ")}`);
					return; // don't advance, acceptance failed
				}
			}
		}

		this.state.currentStepIndex++;
		this.state.updatedAt = new Date().toISOString();

		if (this.state.currentStepIndex >= this.state.steps.length) {
			// All steps done for this item
			const item = this.state.items[this.state.currentItemIndex];
			let result = this.state.results.find((r) => r.item === item);

			if (!result) {
				result = { item, status: "skipped", stepResults: [] };
				this.state.results.push(result);
			}

			if (!result.stepResults.some((s) => s.status === "failed")) {
				if (result.stepResults.length === 0) {
					result.status = "skipped";
				} else {
					result.status = "done";
					// Update module doc to implemented
					this.updateModuleDocStatus(item, "implemented");
				}
			} else {
				result.status = "failed";
			}

			// Move to next item
			this.state.currentItemIndex++;
			this.state.currentStepIndex = 0;
			this.state.retryCount = 0;

			if (this.state.currentItemIndex >= this.state.items.length) {
				this.state.status = "done";
				this.syncRoadmapState();
			}
		}

		savePipelineState(this.cwd, this.state);
	}

	/**
	 * When pipeline completes, sync the roadmap state file to reflect done phases.
	 */
	private syncRoadmapState(): void {
		try {
			const roadmapPath = join(this.cwd, ".pi", ".guardian-roadmap-state.json");
			if (!existsSync(roadmapPath)) return;
			const raw = readFileSync(roadmapPath, "utf-8");
			const roadmap = JSON.parse(raw) as { phases?: { index: number; status: string }[] };
			if (!roadmap.phases) return;
			for (const phase of roadmap.phases) {
				const phaseModules = this.state!.items.filter((item) =>
					item.toLowerCase().includes(`phase${phase.index}`) ||
					this.isItemInPhase(item, phase.index)
				);
				const allDone = phaseModules.every((item) => {
					const r = this.state!.results.find((res) => res.item === item);
					return r && r.status === "done";
				});
				if (allDone && phaseModules.length > 0) {
					phase.status = "done";
				}
			}
			writeFileSync(roadmapPath, JSON.stringify(roadmap, null, 2));
		} catch { /* roadmap file may not exist */ }
	}

	private isItemInPhase(item: string, phaseIndex: number): boolean {
		try {
			const roadmapPath = join(this.cwd, ".pi", "architecture", "implementation-roadmap.md");
			if (!existsSync(roadmapPath)) return false;
			const content = readFileSync(roadmapPath, "utf-8");
			// Check if item appears under the phase section
			const phaseSection = content.match(
				new RegExp(`## Phase ${phaseIndex}:.*?\\n(?:.|\\n)*?(?=\\n## Phase|$)`),
			);
			if (!phaseSection) return false;
			return phaseSection[0].includes(item);
		} catch { return false; }
	}

	/**
	 * Run acceptance gates for a step and return pass/fail.
	 */
	private runAcceptanceGates(step: StepConfig): { allPassed: boolean; errors: string[] } {
		const errors: string[] = [];
		const acceptance = step.acceptance;

		if (acceptance.type === "none") return { allPassed: true, errors: [] };

		if (acceptance.type === "shell") {
			try {
				execFileSync("bash", ["-c", acceptance.command], { cwd: this.cwd, timeout: 300_000, encoding: "utf-8" });
				return { allPassed: true, errors: [] };
			} catch (e: unknown) {
				const err = e as { stdout?: string };
				return { allPassed: false, errors: [err.stdout?.slice(0, 200) || "shell failed"] };
			}
		}

		if (acceptance.type === "validator") {
			for (const validator of acceptance.validators) {
				const scriptPath = VALIDATOR_SCRIPTS[validator];
				if (!scriptPath) { errors.push(`Unknown validator: ${validator}`); continue; }
				try {
					execFileSync("bash", ["-c", scriptPath], { cwd: this.cwd, timeout: 120_000, encoding: "utf-8" });
				} catch (e: unknown) {
					const err = e as { stdout?: string };
					errors.push(`${validator}: ${(err.stdout || "").slice(0, 200)}`);
				}
			}
			return { allPassed: errors.length === 0, errors };
		}

		// LLM gates — can't auto-run, skip verification
		return { allPassed: true, errors: [] };
	}

	markStepFailed(stepName: string, reason: string): void {
		if (!this.state) return;
		const item = this.state.items[this.state.currentItemIndex];
		let result = this.state.results.find((r) => r.item === item);
		if (!result) {
			result = { item, status: "in-progress", stepResults: [] };
			this.state.results.push(result);
		}
		result.stepResults.push({ step: stepName, status: "failed", reason });
		result.status = "failed";
		this.state.updatedAt = new Date().toISOString();
		savePipelineState(this.cwd, this.state);
	}

	markStepPassed(stepName: string): void {
		if (!this.state) return;
		const item = this.state.items[this.state.currentItemIndex];
		let result = this.state.results.find((r) => r.item === item);
		if (!result) {
			result = { item, status: "in-progress", stepResults: [] };
			this.state.results.push(result);
		}
		result.stepResults.push({ step: stepName, status: "passed", reason: "" });
		this.state.updatedAt = new Date().toISOString();
		savePipelineState(this.cwd, this.state);
	}
}

// ── Extension ──

export default function (pi: ExtensionAPI) {
	let manager: PipelineManager | null = null;

	pi.on("session_start", async (_event, ctx) => {
		manager = new PipelineManager(ctx.cwd);
		// Always reconcile pipeline state against ground truth on session start
		manager.reconcile();
		const state = manager.getState();
		if (state && state.status !== "done" && state.status !== "aborted") {
			ctx.ui.setStatus("pipeline", statusLine(state));
		}
	});

	// ── /pipeline command ──
	pi.registerCommand("pipeline", {
		description: "Manage multi-step pipeline workflows",
		handler: async (args, ctx) => {
			if (!manager) manager = new PipelineManager(ctx.cwd);
			const state = manager.getState();

			// pi passes args as a string. Parse into tokens (quote-aware).
			const raw = typeof args === "string" ? args : "";
			const tokens = parseArgs(raw);
			const action = tokens[0];

			// Status
			if (!action || action === "status") {
				if (!state) {
					ctx.ui.notify("No active pipeline. Start one with /pipeline <name> ...", "info");
					return;
				}
				ctx.ui.notify(formatPipelineProgress(state), "info");
				return;
			}

			// Pause
			if (action === "pause") {
				if (!state || state.status !== "running") {
					ctx.ui.notify("No running pipeline to pause.", "warn");
					return;
				}
				manager.pause();
				ctx.ui.notify("⏸ Pipeline paused", "warn");
				ctx.ui.setStatus("pipeline", statusLine(manager.getState()));
				return;
			}

			// Resume
			if (action === "resume") {
				if (!state || state.status !== "paused") {
					ctx.ui.notify("No paused pipeline to resume.", "warn");
					return;
				}
				manager.resume();
				ctx.ui.notify("▶ Pipeline resumed", "success");
				ctx.ui.setStatus("pipeline", statusLine(manager.getState()));
				return;
			}

			// Abort
			if (action === "abort") {
				if (!state || (state.status !== "running" && state.status !== "paused")) {
					ctx.ui.notify("No active pipeline to abort.", "warn");
					return;
				}
				manager.abort();
				ctx.ui.notify("✗ Pipeline aborted", "error");
				ctx.ui.setStatus("pipeline", null);
				return;
			}

			// Skip step
			if (action === "skip-step") {
				if (!state || (state.status !== "running" && state.status !== "paused")) {
					ctx.ui.notify("No active pipeline.", "warn");
					return;
				}
				manager.skipStep();
				ctx.ui.notify("⏭ Step skipped", "info");
				ctx.ui.setStatus("pipeline", statusLine(manager.getState()));
				return;
			}

			// Retry step
			if (action === "retry-step") {
				if (!state || (state.status !== "running" && state.status !== "paused")) {
					ctx.ui.notify("No active pipeline.", "warn");
					return;
				}
				manager.retryStep();
				ctx.ui.notify("🔄 Retrying current step", "info");
				return;
			}

			// Start new pipeline: /pipeline <name> --items "a,b,c" --steps "implement,validate" [--merge-on-valid]
			const name = tokens[0];
			if (!name) {
				ctx.ui.notify(
					'Usage: /pipeline <name> --items "id1,id2" --steps "implement,validate,create-mr" [--merge-on-valid]',
					"error",
				);
				return;
			}

			const itemsFlag = tokens.find((a) => a.startsWith("--items="));
			const stepsFlag = tokens.find((a) => a.startsWith("--steps="));
			const mergeFlag = tokens.includes("--merge-on-valid");

			if (!itemsFlag || !stepsFlag) {
				ctx.ui.notify(
					'Usage: /pipeline <name> --items "id1,id2" --steps "implement,validate,create-mr" [--merge-on-valid]',
					"error",
				);
				return;
			}

			const items = itemsFlag
				.split("=")[1]
				.split(",")
				.map((v) => v.trim())
				.filter(Boolean);
			const stepNames = stepsFlag
				.split("=")[1]
				.split(",")
				.map((v) => v.trim())
				.filter(Boolean);

			// Build step configs from names
			const steps = buildSteps(stepNames);

			const newState = manager.create(name, items, steps, { mergeOnValid: mergeFlag });

			const stepInfo = steps.map((s) => s.name).join(" → ");
			ctx.ui.notify(
				`▶ Pipeline "${name}" started (${newState.id})\n` +
					`Items: ${items.join(", ")}\n` +
					`Steps: ${stepInfo}\n` +
					`${mergeFlag ? "Merge on valid: enabled" : ""}`,
				"success",
			);
			ctx.ui.setStatus("pipeline", statusLine(newState));
		},
	});

	// ── pipeline_status tool ──
	pi.registerTool({
		name: "pipeline_status",
		label: "Pipeline Status",
		description: "Show the current pipeline status and progress.",
		parameters: { type: "object", properties: {} },
		async execute(_toolCallId, _params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new PipelineManager(ctx.cwd);
			manager.reloadFromDisk();
			const state = manager.getState();
			if (!state) {
				return { content: [{ type: "text" as const, text: "No active pipeline." }] };
			}
			return { content: [{ type: "text" as const, text: formatPipelineProgress(state) }] };
		},
	});

	// ── pipeline_advance tool ──
	pi.registerTool({
		name: "pipeline_advance",
		label: "Pipeline Advance",
		description: "Mark current step as passed and advance to the next step/item.",
		parameters: {
			type: "object",
			properties: {
				stepName: { type: "string", description: "Name of the completed step" },
			},
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new PipelineManager(ctx.cwd);
			manager.reloadFromDisk();
			const state = manager.getState();
			if (!state || state.status !== "running") {
				return { content: [{ type: "text" as const, text: "No running pipeline." }] };
			}

			const prevItemIndex = state.currentItemIndex;
			const prevStepIndex = state.currentStepIndex;
			const stepName = (params.stepName as string) || state.steps[prevStepIndex]?.name;

			// Verify current step before advancing (unless user explicitly provides the step name,
			// which counts as manual confirmation)
			if (!params.stepName) {
				const verification = manager.verifyCurrentStep();
				if (!verification.verified) {
					return {
						content: [{ type: "text" as const, text: `Cannot advance: ${verification.reason}` }],
					};
				}
			}

			manager.markStepPassed(stepName);
			manager.advanceStep();

			// Re-read state after advance
			const updatedState = manager.getState()!;

			// Non-blocking: sync progress to tracking issue if one exists
			try {
				const trackingStatePath = join(ctx.cwd, ".pi/.guardian-epic-state.json");
				if (existsSync(trackingStatePath)) {
					const trackingState = JSON.parse(readFileSync(trackingStatePath, "utf-8")) as {
						trackingIssueId?: string | null;
						issues?: { id: string; title: string; status: string }[];
					};
					if (trackingState.trackingIssueId && !trackingState.trackingIssueId.startsWith("local:")) {
						const stepNote = stepName
							? `✓ Step "${stepName}" complete for item "${updatedState.items[prevItemIndex]}"`
							: `✓ Advanced to item ${updatedState.currentItemIndex + 1}/${updatedState.items.length}: "${updatedState.items[updatedState.currentItemIndex]}"`;
						const updateScript = join(ctx.cwd, ".pi/scripts/git/update-tracking-issue.sh");
						if (existsSync(updateScript)) {
							execFileSync("bash", [updateScript, "--id", trackingState.trackingIssueId, "--comment", stepNote], {
								cwd: ctx.cwd,
								timeout: 30_000,
								encoding: "utf-8",
								stdio: "ignore",
							});
						}
					}
				}
			} catch { /* non-blocking — don't fail the pipeline */ }

			// Pipeline complete
			if (updatedState.currentItemIndex >= updatedState.items.length) {
				return {
					content: [{
						type: "text" as const,
						text: `Pipeline complete! All ${updatedState.items.length} items done.`,
					}],
				};
			}

			const currentItem = updatedState.items[updatedState.currentItemIndex];
			const currentStep = updatedState.steps[updatedState.currentStepIndex];
			const movedToNextItem = updatedState.currentItemIndex !== prevItemIndex;

			// If we moved to a new item (completed all steps of previous item),
			// close its remote issue and inject the full next-task prompt
			if (movedToNextItem) {
				// Close remote issue for the completed item
				try {
					const epicStatePath = join(ctx.cwd, ".pi/.guardian-epic-state.json");
					if (existsSync(epicStatePath)) {
						const epicState = JSON.parse(readFileSync(epicStatePath, "utf-8")) as {
							issues?: { id: string; remoteIssueId?: string | null }[];
						};
						const prevItemId = updatedState.items[prevItemIndex];
						const prevIssue = epicState.issues?.find((i) => i.id === prevItemId);
						if (prevIssue?.remoteIssueId) {
							const closeScript = join(ctx.cwd, ".pi/scripts/git/close-issue.sh");
							if (existsSync(closeScript)) {
								execFileSync("bash", [closeScript, "--id", prevIssue.remoteIssueId], {
									cwd: ctx.cwd, timeout: 30_000, encoding: "utf-8", stdio: "ignore",
								});
							}
						}
					}
				} catch { /* non-blocking */ }

				// If the pipeline is fully complete, close tracking issue and epic too
				if (updatedState.currentItemIndex >= updatedState.items.length) {
					try {
						const epicStatePath = join(ctx.cwd, ".pi/.guardian-epic-state.json");
						if (existsSync(epicStatePath)) {
							const epicState = JSON.parse(readFileSync(epicStatePath, "utf-8")) as {
								trackingIssueId?: string | null;
								epicId?: string | null;
							};
							if (epicState.trackingIssueId && !epicState.trackingIssueId.startsWith("local:")) {
								const closeEpicScript = join(ctx.cwd, ".pi/scripts/git/close-epic.sh");
								if (existsSync(closeEpicScript)) {
									const args: string[] = [closeEpicScript, "--tracking-id", epicState.trackingIssueId];
									if (epicState.epicId) args.push("--epic-id", epicState.epicId);
									execFileSync("bash", args, {
										cwd: ctx.cwd, timeout: 30_000, encoding: "utf-8", stdio: "ignore",
									});
								}
							}
						}
					} catch { /* non-blocking */ }
				}
			}

			// If we moved to a new item and the next step is implement,
			// inject the full next-task prompt with issue context
			if (movedToNextItem && currentStep?.name === "implement") {
				// Load epic state for TDD context and remote issue ID
				let epicTdd = false;
				let epicTddTestFiles: string[] = [];
				let remoteId: string | null | undefined;
				try {
					const epicStatePath = join(ctx.cwd, ".pi/.guardian-epic-state.json");
					if (existsSync(epicStatePath)) {
						const epicState = JSON.parse(readFileSync(epicStatePath, "utf-8")) as {
							issues?: { id: string; remoteIssueId?: string | null }[];
							tdd?: boolean;
							tddTestFiles?: string[];
						};
						const issue = epicState.issues?.find((i) => i.id === currentItem);
						remoteId = issue?.remoteIssueId;
						epicTdd = epicState.tdd ?? false;
						epicTddTestFiles = epicState.tddTestFiles ?? [];
					}
				} catch {
					// ignore
				}

				const { content: issueContent, source: issueSource } = fetchIssueContent(
					ctx.cwd,
					currentItem,
					remoteId,
				);

				const tddAdvanceBlock = epicTdd
					? [
						"",
						"## TDD: Red-Green-Refactor",
						"",
						"Failing tests already exist for this component. Follow TDD:",
						"1. **RED:** Run the failing tests first to confirm they fail",
						"2. **GREEN:** Write minimal implementation to make tests pass",
						"3. **REFACTOR:** Clean up while keeping tests green",
						"",
					].join("\n")
					: "";

				const instructions = [
					`## Pipeline: Moving to next item`,
					"",
					`**Pipeline:** ${updatedState.name} (${updatedState.id})`,
					`**Progress:** ${updatedState.currentItemIndex + 1}/${updatedState.items.length} items`,
					`**Issue:** ${issueSource}`,
					"",
					`**Next task:** Item "${currentItem}" → Step: implement`,
					tddAdvanceBlock,
					"",
					"**Instructions:**",
					"1. Review the issue context below",
					epicTdd
						? "2. Run the pre-generated failing tests to confirm RED phase"
						: "2. Implement the component according to the issue spec",
					"3. Run `pipeline_run_acceptance` to validate",
					"4. Call `pipeline_advance` when done",
					"",
					"⚠️ **IMPORTANT:** After you complete this item and call `pipeline_advance`, the pipeline will automatically advance to the next step. Continue this loop until all items are done. Do not stop after completing a single item — keep going through implement → validate → create-mr → merge for every item.",
					"",
					"---",
					"",
					"## Issue Context",
					"",
					issueContent || "Issue content not available.",
				].join("\n");

				return { content: [{ type: "text" as const, text: "Call `pipeline_next_task` to get the next item and continue implementing (no asking).\n\n→ Auto-advance: pipeline_advance (no asking)│" }] };
			}

			// Same item, next step — just report
			const nextInfo = getNextStepInfo(updatedState);
			return { content: [{ type: "text" as const, text: "Call `pipeline_next_task` to continue (no asking).\n\n→ Auto-advance: pipeline_advance (no asking)│" }] };
		},
	});

	// ── pipeline_fail tool ──
	pi.registerTool({
		name: "pipeline_fail",
		label: "Pipeline Fail Step",
		description:
			"Mark current step as failed and advance (skipping remaining steps for this item).",
		parameters: {
			type: "object",
			properties: {
				reason: { type: "string", description: "Why the step failed" },
			},
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new PipelineManager(ctx.cwd);
			manager.reloadFromDisk();
			const state = manager.getState();
			if (!state || state.status !== "running") {
				return { content: [{ type: "text" as const, text: "No running pipeline." }] };
			}

			const reason = (params.reason as string) || "step failed";
			const stepName = state.steps[state.currentStepIndex]?.name;
			manager.markStepFailed(stepName, reason);

			// Skip remaining steps for this item, move to next
			const currentItem = state.items[state.currentItemIndex];
			const remainingSteps = state.steps.slice(state.currentStepIndex + 1);
			for (const step of remainingSteps) {
				manager.markStepFailed(step.name, "skipped due to prior failure");
			}
			state.currentItemIndex++;
			state.currentStepIndex = 0;
			state.retryCount = 0;
			if (state.currentItemIndex >= state.items.length) {
				state.status = "done";
			}
			state.updatedAt = new Date().toISOString();
			savePipelineState(ctx.cwd, state);

			return {
				content: [{ type: "text" as const, text: `Step failed: ${reason}. Moving to next item.` }],
			};
		},
	});

	// ── pipeline_start tool (called by architect extension) ──
	pi.registerTool({
		name: "pipeline_start",
		label: "Pipeline Start",
		description:
			"Start a new pipeline with the given name, items, and steps. Called by the architect extension to begin epic execution.",
		parameters: {
			type: "object",
			properties: {
				name: { type: "string", description: "Pipeline name (usually the epic name)" },
				items: { type: "string", description: "Comma-separated list of issue IDs" },
				steps: { type: "string", description: "Comma-separated list of step names" },
				mergeOnValid: { type: "boolean", description: "Auto-merge if all validators pass" },
			},
			required: ["name", "items", "steps"],
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new PipelineManager(ctx.cwd);
			manager.reloadFromDisk();

			const name = (params.name as string) || "pipeline";
			const items = ((params.items as string) || "")
				.split(",")
				.map((s) => s.trim())
				.filter(Boolean);
			const steps = ((params.steps as string) || "")
				.split(",")
				.map((s) => s.trim())
				.filter(Boolean);

			if (items.length === 0) {
				return { content: [{ type: "text" as const, text: "No items specified." }] };
			}
			if (steps.length === 0) {
				return { content: [{ type: "text" as const, text: "No steps specified." }] };
			}

			const stepConfigs = buildSteps(steps);
			const state = manager.create(name, items, stepConfigs, { mergeOnValid: !!params.mergeOnValid });
			ctx.ui.setStatus(
				"pipeline",
				`▶ ${name} (${state.items.length} items × ${state.steps.length} steps)`,
			);

			let message = `▶ Pipeline "${name}" started\n`;
			message += `Items: ${items.join(", ")}\n`;
			message += `Steps: ${steps.join(" → ")}\n`;
			message += `Total steps: ${items.length * steps.length}\n\n`;
			message += `Current: Item 1/${items.length} → Step 1: ${steps[0]}`;

			return { content: [{ type: "text" as const, text: message }] };
		},
	});

	// ── pipeline_next_task tool ──
	pi.registerTool({
		name: "pipeline_next_task",
		label: "Pipeline Next Task",
		description: "Get the next task prompt with full issue context and step instructions.",
		parameters: {
			type: "object",
			properties: {
				issueId: { type: "string", description: "Issue ID (optional, defaults to current)" },
			},
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new PipelineManager(ctx.cwd);
			manager.reloadFromDisk();
			const state = manager.getState();
			if (!state || state.status !== "running") {
				return { content: [{ type: "text" as const, text: "No running pipeline." }] };
			}
			const issueId = (params.issueId as string) || state.items[state.currentItemIndex];
			const step = state.steps[state.currentStepIndex];
			if (!step) return { content: [{ type: "text" as const, text: "No more steps." }] };

			// Load epic state for TDD context and remote issue ID
			let epicTdd = false;
			let epicTddTestFiles: string[] = [];
			let remoteId: string | null | undefined;
			try {
				const epicStatePath = join(ctx.cwd, ".pi/.guardian-epic-state.json");
				if (existsSync(epicStatePath)) {
					const epicState = JSON.parse(readFileSync(epicStatePath, "utf-8")) as {
						issues?: { id: string; remoteIssueId?: string | null }[];
						tdd?: boolean;
						tddTestFiles?: string[];
					};
					const issue = epicState.issues?.find((i) => i.id === issueId);
					remoteId = issue?.remoteIssueId;
					epicTdd = epicState.tdd ?? false;
					epicTddTestFiles = epicState.tddTestFiles ?? [];
				}
			} catch {
				// ignore
			}

			const { content: issueContent, source: issueSource } = fetchIssueContent(
				ctx.cwd,
				issueId,
				remoteId,
			);

			const stepConfig = buildSteps([step.name])[0];
			let stepPrompt = "";
			if (stepConfig?.prompt) {
				try {
					stepPrompt = readFileSync(join(ctx.cwd, stepConfig.prompt), "utf-8");
				} catch {
					stepPrompt = "// Step prompt not found";
				}
			}

			const tddBlock = epicTdd
				? [
					"",
					"---",
					"",
					"## TDD: Red-Green-Refactor",
					"",
					"Failing test files have already been generated. Follow TDD discipline:",
					"",
					"**1. RED — Run the failing tests first:**",
					...epicTddTestFiles.map((f) => `  - \`${f}\``),
					"",
					"**2. GREEN — Implement the minimum code to make tests pass:**",
					"  - Do NOT write new test files — the tests already exist",
					"  - Focus on the implementation only",
					"  - Run tests after each change to track progress",
					"",
					"**3. REFACTOR — Clean up while keeping tests green:**",
					"  - Extract helpers, improve naming, remove duplication",
					"",
				].join("\n")
				: "";

			const text = [
				"## Pipeline Task",
				"",
				`**Pipeline:** ${state.name} (${state.id})`,
				`**Item:** ${issueId} (${state.currentItemIndex + 1}/${state.items.length})`,
				`**Step:** ${step.name} (${state.currentStepIndex + 1}/${state.steps.length})`,
				`**Issue:** ${issueSource}`,
				"",
				"---",
				"",
				stepPrompt || "",
				tddBlock,
				"",
				"---",
				"",
				"## Issue Context",
				"",
				issueContent,
				"",
				"---",
				"",
				"**Instructions:**",
				"1. Review the issue context above",
				"2. Follow the step prompt instructions",
				"3. When complete, call `pipeline_run_acceptance` to validate your work",
				"4. Then call `pipeline_advance` to move to the next step (no asking)",
				"",
				"→ Auto-advance: pipeline_advance (no asking)│",
			].join("\n");

			return { content: [{ type: "text" as const, text }] };
		},
	});

	// ── pipeline_run_acceptance tool ──
	pi.registerTool({
		name: "pipeline_run_acceptance",
		label: "Pipeline Run Acceptance",
		description: "Run the acceptance gate validators for the current step.",
		parameters: { type: "object", properties: {} },
		async execute(_toolCallId, _params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new PipelineManager(ctx.cwd);
			manager.reloadFromDisk();
			const state = manager.getState();
			if (!state || state.status !== "running") {
				return { content: [{ type: "text" as const, text: "No running pipeline." }] };
			}
			const step = state.steps[state.currentStepIndex];
			if (!step) return { content: [{ type: "text" as const, text: "No current step." }] };

			const acceptance = step.acceptance;
			if (acceptance.type === "none") {
				manager.markStepPassed(step.name);
				return {
					content: [{ type: "text" as const, text: `Step "${step.name}" passed (no gate).` }],
				};
			}

			if (acceptance.type === "shell") {
				const lines: string[] = [`## Acceptance Gate: ${step.name}\n`];
				const scriptPath = acceptance.command;
				const fullPath = join(ctx.cwd, scriptPath);
				if (!existsSync(fullPath)) {
					lines.push("Shell script not found: " + scriptPath);
					lines.push("Call pipeline_advance to skip.");
					return { content: [{ type: "text" as const, text: lines.join("\n") }] };
				}
				try {
					const output = execFileSync("bash", [scriptPath], {
						cwd: ctx.cwd,
						timeout: 300_000,
						encoding: "utf-8",
					});
					lines.push("Script PASS: " + scriptPath);
					lines.push("```\n" + output + "\n```");
					manager.markStepPassed(step.name);
					lines.push("\n**Result: PASSED**");
				} catch (e: unknown) {
					const err = e as { stdout?: string };
					lines.push("Script FAIL: " + scriptPath);
					lines.push("```\n" + ((err.stdout || "").split("\n").slice(-10).join("\n")) + "\n```");
				}
				return { content: [{ type: "text" as const, text: lines.join("\n") }] };
			}

			if (acceptance.type !== "validator") {
				return { content: [{ type: "text" as const, text: "Unknown acceptance type: " + acceptance.type }] };
			}

			const lines: string[] = ["## Acceptance Gate: " + step.name + "\n"];
			let allPassed = true;

			for (const validator of acceptance.validators) {
				const scriptPath = VALIDATOR_SCRIPTS[validator];
				if (!scriptPath) {
					lines.push(`### ${validator}: UNKNOWN`);
					lines.push(`  Validator not found: ${validator}`);
					allPassed = false;
					continue;
				}
				const fullPath = join(ctx.cwd, scriptPath);
				if (!existsSync(fullPath)) {
					lines.push(`### ${validator}: SKIPPED`);
					lines.push("  Script not found");
					continue;
				}
				try {
					execFileSync("bash", ["-c", scriptPath], {
						cwd: ctx.cwd,
						timeout: 120_000,
						encoding: "utf-8",
					});
					lines.push(`### ${validator}: PASS`);
				} catch (e: unknown) {
					const err = e as { stdout?: string };
					lines.push(`### ${validator}: FAIL`);
					lines.push(`\`\`\`${(err.stdout || "").split("\n").slice(-10).join("\n")}\`\`\``);
					allPassed = false;
				}
			}

			if (allPassed) {
				manager.markStepPassed(step.name);
				lines.push("\n**Result: ALL VALIDATORS PASSED**");
				lines.push("Call pipeline_advance to move to the next step.");
			} else {
				lines.push("\n**Result: SOME VALIDATORS FAILED**");
				lines.push("Fix the issues and run pipeline_run_acceptance again.");
			}
			return { content: [{ type: "text" as const, text: lines.join("\n") }] };
		},
	});
}

// ── Step Builder ──

function buildSteps(stepNames: string[]): StepConfig[] {
	const stepConfigs: Record<string, StepConfig> = {
		implement: {
			name: "implement",
			prompt: ".pi/prompts/issue-implementation-series.md",
			acceptance: { type: "validator", validators: ["ci"] },
		},
		validate: {
			name: "validate",
			acceptance: { type: "validator", validators: ["ci", "tests", "security"] },
		},
		"create-mr": {
			name: "create-mr",
			prompt: ".pi/prompts/issue-closeout.md",
			acceptance: { type: "none" },
		},
		merge: {
			name: "merge",
			acceptance: { type: "validator", validators: ["ci", "canonical"] },
		},
		document: {
			name: "document",
			prompt: ".pi/prompts/blueprint-update.md",
			acceptance: { type: "validator", validators: ["canonical"] },
		},
		test: {
			name: "test",
			acceptance: { type: "validator", validators: ["tests"] },
		},
		"security-review": {
			name: "security-review",
			acceptance: { type: "validator", validators: ["security"] },
		},
	};

	return stepNames.map((name) => {
		const config = stepConfigs[name];
		if (config) return { ...config };
		// Unknown step: no prompt, no acceptance gate
		return { name, acceptance: { type: "none" } as AcceptanceConfig };
	});
}

function getNextStepInfo(state: PipelineState): string {
	if (state.currentItemIndex >= state.items.length) {
		return "Pipeline complete! All items processed.";
	}
	const item = state.items[state.currentItemIndex];
	const step = state.steps[state.currentStepIndex];
	if (!step) return "No more steps.";
	return `Next: Item "${item}" → Step "${step.name}" (${state.currentStepIndex + 1}/${state.steps.length})`;
}
