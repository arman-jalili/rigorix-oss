/**
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

import { execSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

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

function runScript(cwd: string, script: string): { exitCode: number; stdout: string } {
	try {
		const stdout = execSync(`bash -c "${script}"`, { cwd, timeout: 120_000, encoding: "utf-8" });
		return { exitCode: 0, stdout };
	} catch (e: unknown) {
		const err = e as { status?: number; stdout?: string; message?: string };
		return { exitCode: err.status ?? 1, stdout: err.stdout ?? err.message ?? "" };
	}
}

function readRepository(cwd: string): string | null {
	try {
		const manifestPath = join(cwd, "guardian-manifest.json");
		if (existsSync(manifestPath)) {
			const raw = readFileSync(manifestPath, "utf-8");
			const manifest = JSON.parse(raw) as {
				repository?: string;
				templateContext?: { repository?: string };
			};
			if (manifest.repository) return manifest.repository;
			if (manifest.templateContext?.repository) return manifest.templateContext.repository;
		}
	} catch {
		// ignore
	}
	return null;
}

function readRepoTool(cwd: string): string {
	try {
		const manifestPath = join(cwd, "guardian-manifest.json");
		if (existsSync(manifestPath)) {
			const raw = readFileSync(manifestPath, "utf-8");
			const manifest = JSON.parse(raw) as { repoTool?: string };
			if (manifest.repoTool === "glab") return "glab";
		}
	} catch {
		// fall through to default
	}
	return "gh";
}

function getGitBaseUrl(repoTool: string): string {
	if (repoTool === "glab") {
		try {
			const uri = execSync("glab config get gitlab_uri 2>/dev/null", {
				encoding: "utf-8",
			}).trim();
			if (uri) return uri.replace(/\/+$/, "");
		} catch {
			// fall through to default
		}
		return "https://gitlab.com";
	}
	return "https://github.com";
}

// Fetch issue content from remote (gh or glab) or fallback to local file
function fetchIssueContent(
	cwd: string,
	issueId: string,
	remoteIssueId?: string | null,
): { content: string; source: string } {
	const repository = readRepository(cwd);
	const repoTool = readRepoTool(cwd);
	const baseUrl = getGitBaseUrl(repoTool);

	if (remoteIssueId && repository) {
		try {
			let result;
			if (repoTool === "glab") {
				result = runScript(
					cwd,
					`glab issue view ${remoteIssueId} --repo ${repository} --output json`,
				);
				if (result.exitCode === 0 && result.stdout) {
					const parsed = JSON.parse(result.stdout) as {
						title?: string;
						description?: string;
					};
					if (parsed.description) {
						return {
							content: parsed.description,
							source: `Remote issue: ${baseUrl}/${repository}/issues/${remoteIssueId}`,
						};
					}
				}
			} else {
				result = runScript(
					cwd,
					`gh issue view ${remoteIssueId} --repo ${repository} --json title,body`,
				);
				if (result.exitCode === 0 && result.stdout) {
					const parsed = JSON.parse(result.stdout) as { title?: string; body?: string };
					if (parsed.body) {
						return {
							content: parsed.body,
							source: `Remote issue: ${baseUrl}/${repository}/issues/${remoteIssueId}`,
						};
					}
				}
			}
		} catch {
			// fallback to local file
		}
	}

	// Fallback to local file
	const issueFilename = `${issueId}.md`.replace(/\//g, "-");
	const issuePath = join(cwd, ".pi/issues", issueFilename);
	try {
		if (existsSync(issuePath)) {
			return {
				content: readFileSync(issuePath, "utf-8"),
				source: `Local file: .pi/issues/${issueFilename}`,
			};
		}
	} catch {
		// ignore
	}

	return {
		content: "Issue content not available.",
		source: issueId,
	};
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

	constructor(private cwd: string) {
		this.state = loadPipelineState(cwd);
	}

	getState(): PipelineState | null {
		return this.state;
	}

	reload(): void {
		const raw = loadPipelineState(this.cwd);
		if (raw) {
			// Migrate old string-step format to StepConfig objects
			if (raw.steps.length > 0 && typeof raw.steps[0] === "string") {
				raw.steps = buildSteps(raw.steps as unknown as string[]);
			}
		}
		this.state = raw;
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
		this.state.currentStepIndex++;
		this.state.updatedAt = new Date().toISOString();

		if (this.state.currentStepIndex >= this.state.steps.length) {
			// All steps done for this item
			const item = this.state.items[this.state.currentItemIndex];
			let result = this.state.results.find((r) => r.item === item);

			// If no result entry exists (e.g. advanceStep called before any steps ran),
			// create one so the item is tracked.
			if (!result) {
				result = { item, status: "skipped", stepResults: [] };
				this.state.results.push(result);
			}

			if (!result.stepResults.some((s) => s.status === "failed")) {
				if (result.stepResults.length === 0) {
					result.status = "skipped";
				} else {
					result.status = "done";
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
			}
		}

		savePipelineState(this.cwd, this.state);
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

// ── Step-specific instruction builders ──

function getDefaultBranch(cwd: string): string {
	try {
		const symRef = execSync("git symbolic-ref refs/remotes/origin/HEAD", {
			cwd, encoding: "utf-8"
		}).trim();
		return symRef.replace(/^refs\/remotes\/origin\//, "");
	} catch {
		return "main";
	}
}

function buildImplementInstructions(issueId: string, branchExists: boolean): string {
	const slug = issueId.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
	const branch = `feat/${slug}`;

	if (branchExists) {
		return [
			"## MANDATORY GIT WORKFLOW",
			"",
			`Branch \`${branch}\` already exists. Continue working on it.`,
			"",
			"1. **CHECKOUT:** `git checkout " + branch + "`",
			"2. Implement the issue requirements",
			"3. **COMMIT every logical chunk:** `git add <files> && git commit -m \"feat: description\"`",
			"4. **PUSH regularly:** `git push origin " + branch + "`",
			"5. When implementation complete, commit+push final changes then call `pipeline_run_acceptance`",
			"",
			"⛔ DO NOT skip git. Every commit must be pushed. This is MANDATORY.",
		].join("\n");
	}

	return [
		"## MANDATORY GIT WORKFLOW — DO NOT SKIP",
		"",
		"1. **CREATE BRANCH:** `git checkout -b " + branch + "`",
		"2. Implement the issue requirements",
		"3. **COMMIT every logical chunk:** `git add <files> && git commit -m \"feat: description\"`",
		"4. **PUSH regularly:** `git push origin " + branch + "`",
		"5. When implementation complete, commit+push final changes then call `pipeline_run_acceptance`",
		"",
		"⛔ **CRITICAL:** You MUST create a branch, commit, and push. The pipeline validates git history.",
		"Do NOT implement directly on main. Do NOT skip git operations.",
	].join("\n");
}

function buildCreateMRInstructions(issueId: string, repo: string, defaultBranch: string): string {
	const slug = issueId.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
	const branch = `feat/${slug}`;

	return [
		"## MANDATORY: CREATE PULL REQUEST",
		"",
		"1. **ENSURE pushed:** `git push origin " + branch + "`",
		"2. **CREATE PR:** `gh pr create --base " + defaultBranch + " --head " + branch + " --title \"" + issueId + ": <description>\" --body \"Closes #<issue-number>\"`",
		"3. Wait for CI checks to pass on the PR",
		"4. If checks fail, fix, commit+push, re-check",
		"5. When PR is created and CI passes, call `pipeline_advance`",
		"",
		"⛔ **CRITICAL:** You MUST create a PR via `gh pr create`. Do NOT skip this.",
		"The repository is: " + (repo || "check guardian-manifest.json"),
	].join("\n");
}

function buildMergeInstructions(issueId: string, defaultBranch: string): string {
	const slug = issueId.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
	const branch = `feat/${slug}`;

	return [
		"## MANDATORY: MERGE PULL REQUEST",
		"",
		"1. Find the PR for branch `" + branch + "`: `gh pr list --head " + branch + " --json number --jq '.[0].number'`",
		"2. **MERGE PR:** `gh pr merge <PR_NUMBER> --squash --delete-branch`",
		"3. **CHECKOUT main:** `git checkout " + defaultBranch + "`",
		"4. **PULL latest:** `git pull origin " + defaultBranch + "`",
		"5. Call `pipeline_advance`",
		"",
		"⛔ **CRITICAL:** You MUST merge the PR via `gh pr merge`. Do NOT skip this.",
	].join("\n");
}

// ── Extension ──

export default function (pi: ExtensionAPI) {
	let manager: PipelineManager | null = null;

	pi.on("session_start", async (_event, ctx) => {
		manager = new PipelineManager(ctx.cwd);
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
			manager.reload();
			const state = manager.getState();

			// pi passes args as a string. Split into tokens.
			const raw = typeof args === "string" ? args : "";
			const tokens = raw.split(/\s+/).filter(Boolean);
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
			manager.reload();
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
			manager.reload();
			const state = manager.getState();
			if (!state || state.status !== "running") {
				return { content: [{ type: "text" as const, text: "No running pipeline." }] };
			}

			const prevItemIndex = state.currentItemIndex;
			const prevStepIndex = state.currentStepIndex;
			const stepName = (params.stepName as string) || state.steps[prevStepIndex]?.name;
			manager.markStepPassed(stepName);
			manager.advanceStep();

			// Re-read state after advance
			const updatedState = manager.getState()!;

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
			// inject the full next-task prompt with issue context
			if (movedToNextItem && currentStep?.name === "implement") {
				// Find the remote issue ID from epic state
				let remoteId: string | null | undefined;
				try {
					const epicStatePath = join(ctx.cwd, ".pi/.guardian-epic-state.json");
					if (existsSync(epicStatePath)) {
						const epicState = JSON.parse(readFileSync(epicStatePath, "utf-8")) as {
							issues?: { id: string; remoteIssueId?: string | null }[];
						};
						const issue = epicState.issues?.find((i) => i.id === currentItem);
						remoteId = issue?.remoteIssueId;
					}
				} catch {
					// ignore
				}

				const { content: issueContent, source: issueSource } = fetchIssueContent(
					ctx.cwd,
					currentItem,
					remoteId,
				);

				const instructions = [
					`## Pipeline: Moving to next item`,
					"",
					`**Pipeline:** ${updatedState.name} (${updatedState.id})`,
					`**Progress:** ${updatedState.currentItemIndex + 1}/${updatedState.items.length} items`,
					`**Issue:** ${issueSource}`,
					"",
					`**Next task:** Item "${currentItem}" → Step: implement`,
					"",
					"**Instructions:**",
					"1. Create branch: `feat/${currentItem}`",
					"2. Review the issue context below",
					"3. Implement the component according to the issue spec",
					"4. Run `pipeline_run_acceptance` to validate",
					"5. Call `pipeline_advance` when done",
					"",
					"**Available tools:**",
					"- `pipeline_next_task` — get full context for current step",
					"- `pipeline_run_acceptance` — run validators (CI, tests, security, shell, LLM)",
					"- `pipeline_advance` — mark step passed, advance to next",
					"- `pipeline_fail` — mark step failed, skip remaining steps for this item",
					"- `pipeline_status` — check overall pipeline progress",
					"",
					"⚠️ **IMPORTANT:** After each step, call `pipeline_run_acceptance` then `pipeline_advance`. The pipeline flows: implement → validate → create-mr → merge. Continue through ALL items — do not stop after completing one.",
					"",
					"---",
					"",
					"## Issue Context",
					"",
					issueContent || "Issue content not available.",
				].join("\n");

				return { content: [{ type: "text" as const, text: instructions }] };
			}

			// Same item, next step — just report
			const nextInfo = getNextStepInfo(updatedState);
			return { content: [{ type: "text" as const, text: nextInfo }] };
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
			manager.reload();
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

			const state = manager.create(name, items, steps, { mergeOnValid: !!params.mergeOnValid });
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
			manager.reload();
			const state = manager.getState();
			if (!state || state.status !== "running") {
				return { content: [{ type: "text" as const, text: "No running pipeline." }] };
			}
			const issueId = (params.issueId as string) || state.items[state.currentItemIndex];
			const step = state.steps[state.currentStepIndex];
			if (!step) return { content: [{ type: "text" as const, text: "No more steps." }] };

			// Find the remote issue ID from epic state
			let remoteId: string | null | undefined;
			try {
				const epicStatePath = join(ctx.cwd, ".pi/.guardian-epic-state.json");
				if (existsSync(epicStatePath)) {
					const epicState = JSON.parse(readFileSync(epicStatePath, "utf-8")) as {
						issues?: { id: string; remoteIssueId?: string | null }[];
					};
					const issue = epicState.issues?.find((i) => i.id === issueId);
					remoteId = issue?.remoteIssueId;
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

			// Build step-specific git-mandatory instructions
			const repo = readRepository(ctx.cwd) || "";
			const defaultBranch = getDefaultBranch(ctx.cwd);
			const slug = issueId.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
			const branch = `feat/${slug}`;

			let stepInstructions = "";
			if (step.name === "implement") {
				let branchExists = false;
				try {
					execSync(`git rev-parse --verify ${branch}`, { cwd: ctx.cwd, encoding: "utf-8" });
					branchExists = true;
				} catch { /* doesn't exist */ }
				stepInstructions = buildImplementInstructions(issueId, branchExists);
			} else if (step.name === "validate") {
				stepInstructions = "## VALIDATION STEP\n\n1. Ensure all changes committed+push to feature branch\n2. Call `pipeline_run_acceptance`\n3. If pass: `pipeline_advance`. If fail: fix, commit+push, re-run.\n\n⛔ All changes must be committed and pushed before validation.";
			} else if (step.name === "create-mr") {
				stepInstructions = buildCreateMRInstructions(issueId, repo, defaultBranch);
			} else if (step.name === "merge") {
				stepInstructions = buildMergeInstructions(issueId, defaultBranch);
			} else {
				stepInstructions = "## Instructions\n\n1. Complete the work for this step\n2. Commit and push all changes\n3. Call `pipeline_run_acceptance` then `pipeline_advance`";
			}

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
				stepInstructions,
				"",
				"---",
				"",
				stepPrompt || "",
				"",
				"---",
				"",
				"## Issue Context",
				"",
				issueContent,
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
			manager.reload();
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

			if (acceptance.type === "llm") {
				const promptPath = join(ctx.cwd, acceptance.prompt);
				let validatorPrompt = "";
				try {
					if (existsSync(promptPath)) {
						validatorPrompt = readFileSync(promptPath, "utf-8");
					}
				} catch {
					// ignore
				}
				if (!validatorPrompt) {
					return {
						content: [{
							type: "text" as const,
							text: `LLM validator prompt not found: ${acceptance.prompt}. Skipping gate.`,
						}],
					};
				}
				return {
					content: [{
						type: "text" as const,
						text: [
							`## LLM Validator: ${step.name}`,
							"",
							"Read the validator agent definition below. Execute it as an agent:",
							"1. Read and understand the validation criteria",
							"2. Audit the current implementation against each criterion",
							"3. Report pass/fail with evidence for each criterion",
							"4. If all pass, call `pipeline_advance`",
							"5. If any fail, fix the issues and re-run acceptance",
							"",
							"---",
							"",
							validatorPrompt,
						].join("\n"),
					}],
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
					const output = execSync("bash " + scriptPath, {
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
					execSync(`bash -c "${scriptPath}"`, {
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
			prompt: ".pi/prompts/issue-merge.md",
			acceptance: { type: "validator", validators: ["ci", "canonical"] },
		},
		"architecture-validator": {
			name: "architecture-validator",
			acceptance: { type: "llm", prompt: ".pi/agents/architecture-validator.md" },
		},
		"security-validator": {
			name: "security-validator",
			acceptance: { type: "llm", prompt: ".pi/agents/security-validator.md" },
		},
		"operations-validator": {
			name: "operations-validator",
			acceptance: { type: "llm", prompt: ".pi/agents/operations-validator.md" },
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
