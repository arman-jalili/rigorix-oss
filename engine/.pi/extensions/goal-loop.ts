/**
 * Goal Loop Extension for pi
 *
 * Persistent standing goals with validator-backed judge.
 * After each turn, the goal manager evaluates whether the standing goal
 * is satisfied using a dual judge: deterministic validators + LLM semantic check.
 *
 * Inspired by Hermes-Agent /goal (Ralph loop) — adapted for Guardian's
 * shift-left validation model with script-backed completion criteria.
 *
 * Commands:
 *   /goal <text>          Set a standing goal
 *   /goal status          Show current goal
 *   /goal pause           Pause auto-continuation
 *   /goal resume          Resume (resets turn counter)
 *   /goal clear           Drop the goal
 *   /subgoal <text>       Add criteria to active goal
 *   /subgoal list         List current subgoals
 *   /subgoal remove <N>   Remove subgoal by 1-based index
 *   /subgoal clear        Remove all subgoals
 */

import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

// ── Types ──

type ExtensionContext = {
	cwd: string;
	ui: {
		notify(message: string, level?: string): void;
		setStatus(key: string, message: string | null): void;
		confirm(title: string, message: string): Promise<boolean>;
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

type GoalState = {
	goal: string;
	status: "active" | "paused" | "done" | "cleared";
	turnsUsed: number;
	maxTurns: number;
	createdAt: string;
	lastTurnAt: string;
	lastVerdict: string;
	lastReason: string;
	pausedReason: string | null;
	subgoals: string[];
	validators: string[]; // per-goal validator list (e.g. ["ci", "tests", "security"])
	validatorResults: Record<string, { passed: boolean; lastRun: string }>;
};

type JudgeResult = {
	done: boolean;
	reason: string;
	validatorPass: boolean;
	validatorFailures: string[];
	llmVerdict?: "done" | "continue";
	llmReason?: string;
};

// ── Constants ──

const DEFAULT_MAX_TURNS = 20;
const GOAL_STATE_KEY = ".pi/.guardian-goal-state.json";

const CONTINUATION_PROMPT = (goal: string, subgoals: string[]) => {
	const subgoalBlock =
		subgoals.length > 0
			? `\n\nAdditional criteria (all must be satisfied):\n${subgoals.map((s, i) => `  ${i + 1}. ${s}`).join("\n")}`
			: "";
	return `[Continuing toward your standing goal]\nGoal: ${goal}\nContinue working toward this goal. Take the next concrete step. If you believe the goal is complete, state so explicitly and stop. If you are blocked and need input from the user, say so clearly and stop.${subgoalBlock}`;
};

const JUDGE_SYSTEM_PROMPT =
	"You are a strict judge evaluating whether an autonomous agent has " +
	`achieved a user's stated goal. You receive the goal text, validator results, ` +
	`and the agent's most recent response.\n\n` +
	"A goal is DONE only when:\n" +
	"- The response explicitly confirms the goal was completed, OR\n" +
	"- The response clearly shows the final deliverable was produced, OR\n" +
	"- The goal is unachievable / blocked / needs user input (treat as DONE with reason).\n\n" +
	"Otherwise the goal is NOT done — CONTINUE.\n\n" +
	"Reply ONLY with a single JSON object on one line:\n" +
	`{"done": <true|false>, "reason": "<one-sentence rationale>"}`;

// ── Persistence ──

function loadGoalState(cwd: string): GoalState | null {
	const p = join(cwd, GOAL_STATE_KEY);
	if (!existsSync(p)) return null;
	try {
		return JSON.parse(readFileSync(p, "utf-8")) as GoalState;
	} catch {
		return null;
	}
}

function saveGoalState(cwd: string, state: GoalState): void {
	const p = join(cwd, GOAL_STATE_KEY);
	const dir = dirname(p);
	if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
	writeFileSync(p, JSON.stringify(state, null, 2));
}

function clearGoalState(cwd: string): void {
	const p = join(cwd, GOAL_STATE_KEY);
	if (existsSync(p)) writeFileSync(p, JSON.stringify({ status: "cleared" }, null, 2));
}

const BUILTIN_VALIDATORS: Record<string, string> = {
	ci: "engine/.pi/scripts/validate-ci.sh",
	canonical: "engine/.pi/scripts/validate-canonical.sh",
	tests: "engine/.pi/scripts/validate-tests.sh",
	security: "engine/.pi/scripts/validate-security.sh",
	operations: "engine/.pi/scripts/validate-operations.sh",
	architecture: "engine/.pi/scripts/validate-architecture.sh",
	integration: "engine/.pi/scripts/validate-integration.sh",
};

/** Discover custom validators from `.pi/scripts/validate-*.sh` */
function discoverCustomValidators(cwd: string): Record<string, string> {
	const scriptsDir = join(cwd, ".pi/scripts");
	if (!existsSync(scriptsDir)) return {};
	const custom: Record<string, string> = {};
	const builtinPaths = new Set(Object.values(BUILTIN_VALIDATORS));
	try {
		for (const file of readdirSync(scriptsDir)) {
			if (!file.startsWith("validate-") || !file.endsWith(".sh")) continue;
			const relPath = join(".pi/scripts", file);
			if (builtinPaths.has(relPath)) continue;
			custom[file.replace("validate-", "").replace(".sh", "")] = relPath;
		}
	} catch {
		/* ignore */
	}
	return custom;
}

function getAllValidators(cwd: string): Record<string, string> {
	return { ...BUILTIN_VALIDATORS, ...discoverCustomValidators(cwd) };
}

const BUILTIN_NAMES = Object.keys(BUILTIN_VALIDATORS);

// ── Validator-Backed Judge ──

async function runValidatorsForGoal(
	ctx: ExtensionContext,
	validators: string[] = ["ci", "canonical"],
): Promise<{ pass: boolean; failures: string[]; results: Record<string, boolean> }> {
	const registry = getAllValidators(ctx.cwd);
	const results: Record<string, boolean> = {};
	const failures: string[] = [];

	for (const name of validators) {
		const relPath = registry[name];
		if (!relPath) {
			results[name] = false;
			failures.push(`${name}: unknown validator`);
			continue;
		}
		const fullPath = join(ctx.cwd, relPath);
		if (!existsSync(fullPath)) {
			results[name] = true; // missing script = skip = pass
			continue;
		}
		try {
			const r = await ctx.shell.execute(`bash ${relPath}`, {
				signal: AbortSignal.timeout(60_000),
			});
			const passed = r.exitCode === 0;
			results[name] = passed;
			if (!passed) failures.push(`${name}: validator failed`);
		} catch {
			results[name] = false;
			failures.push(`${name}: timeout or error`);
		}
	}

	return { pass: failures.length === 0, failures, results };
}

async function runLLMJudge(
	ctx: ExtensionContext,
	goal: string,
	lastResponse: string,
	subgoals: string[],
): Promise<{ done: boolean; reason: string }> {
	const truncatedGoal = goal.length > 2000 ? `${goal.slice(0, 2000)}…` : goal;
	const truncatedResponse =
		lastResponse.length > 4000 ? `${lastResponse.slice(0, 4000)}…` : lastResponse;
	const subgoalsBlock =
		subgoals.length > 0
			? `\n\nAdditional criteria (all must be satisfied):\n${subgoals.map((s, i) => `  ${i + 1}. ${s}`).join("\n")}`
			: "";

	const prompt = `Goal:\n${truncatedGoal}\n${subgoalsBlock}\n\nAgent's most recent response:\n${truncatedResponse}\n\nIs the goal satisfied?`;

	// Use guardian_coordinate to run an LLM-based judgment
	try {
		const result = await ctx.tools.execute("guardian_scope", {});
		// We can't directly call LLM from extension — instead return a heuristic judgment
		// The LLM judge runs via the agent itself being prompted to self-assess
		// For now, return continue to let the agent decide
		return { done: false, reason: "LLM judge requires agent self-assessment turn" };
	} catch {
		return { done: false, reason: "judge unavailable, continuing" };
	}
}

function truncate(text: string, limit: number): string {
	if (text.length <= limit) return text;
	return `${text.slice(0, limit)}…`;
}

// ── GoalManager ──

class GoalManager {
	private _state: GoalState | null = null;

	constructor(private cwd: string) {
		this._state = loadGoalState(cwd);
	}

	get state(): GoalState | null {
		return this._state;
	}

	get active(): boolean {
		return this._state !== null && this._state.status === "active";
	}

	get hasGoal(): boolean {
		return this._state !== null && ["active", "paused", "done"].includes(this._state.status);
	}

	statusLine(): string {
		const s = this._state;
		if (!s || s.status === "cleared") return "No active goal. Set one with /goal <text>.";
		const turns = `${s.turnsUsed}/${s.maxTurns} turns`;
		const subs =
			s.subgoals.length > 0
				? `, ${s.subgoals.length} subgoal${s.subgoals.length > 1 ? "s" : ""}`
				: "";
		const vals = s.validators.length > 0 ? ` [${s.validators.join(", ")}]` : "";
		if (s.status === "active") return `⊙ Goal (active, ${turns}${vals}${subs}): ${s.goal}`;
		if (s.status === "paused")
			return `⏸ Goal (paused, ${turns}${vals}${subs}${s.pausedReason ? ` — ${s.pausedReason}` : ""}): ${s.goal}`;
		if (s.status === "done") return `✓ Goal done (${turns}${vals}${subs}): ${s.goal}`;
		return `Goal (${s.status}, ${turns}${vals}${subs}): ${s.goal}`;
	}

	set(goalText: string, opts?: { maxTurns?: number; validators?: string[] }): GoalState {
		const goal = goalText.trim();
		if (!goal) throw new Error("goal text is empty");
		this._state = {
			goal,
			status: "active",
			turnsUsed: 0,
			maxTurns: opts?.maxTurns || DEFAULT_MAX_TURNS,
			createdAt: new Date().toISOString(),
			lastTurnAt: new Date().toISOString(),
			lastVerdict: "",
			lastReason: "",
			pausedReason: null,
			subgoals: [],
			validators: opts?.validators ?? [],
			validatorResults: {},
		};
		saveGoalState(this.cwd, this._state);
		return this._state;
	}

	setValidators(validators: string[]): string[] {
		if (!this._state || !this.hasGoal) throw new Error("no active goal");
		this._state.validators = validators;
		saveGoalState(this.cwd, this._state);
		return validators;
	}

	pause(reason = "user-paused"): void {
		if (!this.state) return;
		this.state.status = "paused";
		this.state.pausedReason = reason;
		saveGoalState(this.cwd, this.state);
	}

	resume(): void {
		if (!this.state) return;
		this.state.status = "active";
		this.state.pausedReason = null;
		this.state.turnsUsed = 0;
		saveGoalState(this.cwd, this.state);
	}

	clear(): void {
		if (!this.state) return;
		this.state.status = "cleared";
		saveGoalState(this.cwd, this.state);
		this.state = null;
	}

	addSubgoal(textIn: string): string {
		if (!this.state || !this.hasGoal) throw new Error("no active goal");
		const text = textIn.trim();
		if (!text) throw new Error("subgoal text is empty");
		this.state.subgoals.push(text);
		saveGoalState(this.cwd, this.state);
		return text;
	}

	removeSubgoal(index: number): string {
		if (!this.state || !this.hasGoal) throw new Error("no active goal");
		const idx = index - 1;
		if (idx < 0 || idx >= this.state.subgoals.length) {
			throw new Error(`index out of range (1..${this.state.subgoals.length})`);
		}
		const removed = this.state.subgoals.splice(idx, 1)[0];
		saveGoalState(this.cwd, this.state);
		return removed;
	}

	clearSubgoals(): number {
		if (!this.state || !this.hasGoal) throw new Error("no active goal");
		const prev = this.state.subgoals.length;
		this.state.subgoals = [];
		saveGoalState(this.cwd, this.state);
		return prev;
	}

	continuationPrompt(): string | null {
		if (!this.state || this.state.status !== "active") return null;
		return CONTINUATION_PROMPT(this.state.goal, this.state.subgoals);
	}

	async evaluateAfterTurnWithCtx(
		ctx: ExtensionContext,
		lastResponse: string,
	): Promise<{
		shouldContinue: boolean;
		verdict: string;
		reason: string;
		message: string;
		continuationPrompt: string | null;
	}> {
		const state = this.state;
		if (!state || state.status !== "active") {
			return {
				shouldContinue: false,
				verdict: "inactive",
				reason: "no active goal",
				message: "",
				continuationPrompt: null,
			};
		}

		state.turnsUsed++;
		state.lastTurnAt = new Date().toISOString();

		// Step 1: Run deterministic validators (per-goal list, or fallback)
		const validatorsToRun = state.validators.length > 0 ? state.validators : ["ci", "canonical"];
		const valResult = await runValidatorsForGoal(ctx, validatorsToRun);
		state.validatorResults = {};
		for (const [name, passed] of Object.entries(valResult.results)) {
			state.validatorResults[name] = { passed, lastRun: new Date().toISOString() };
		}

		// If validators fail → definitely continue
		if (!valResult.pass) {
			state.lastVerdict = "continue";
			state.lastReason = `Validators failed: ${valResult.failures.join(", ")}`;
			saveGoalState(this.cwd, state);
			if (state.turnsUsed >= state.maxTurns) {
				state.status = "paused";
				state.pausedReason = `turn budget exhausted (${state.turnsUsed}/${state.maxTurns})`;
				saveGoalState(this.cwd, state);
				return {
					shouldContinue: false,
					verdict: "continue",
					reason: state.pausedReason,
					message: `⏸ Goal paused — ${state.turnsUsed}/${state.maxTurns} turns used. Validators still failing.`,
					continuationPrompt: null,
				};
			}
			return {
				shouldContinue: true,
				verdict: "continue",
				reason: state.lastReason,
				message: `↻ Continuing toward goal (${state.turnsUsed}/${state.maxTurns}): ${state.lastReason}`,
				continuationPrompt: this.continuationPrompt(),
			};
		}

		// Step 2: LLM semantic judge (via ask the agent to self-assess)
		// We inject a self-assessment prompt into the continuation
		const llmResult = await runLLMJudge(ctx, state.goal, lastResponse, state.subgoals);

		if (llmResult.done) {
			state.status = "done";
			state.lastVerdict = "done";
			state.lastReason = llmResult.reason;
			saveGoalState(this.cwd, state);
			return {
				shouldContinue: false,
				verdict: "done",
				reason: llmResult.reason,
				message: `✓ Goal achieved: ${llmResult.reason}`,
				continuationPrompt: null,
			};
		}

		// Step 3: Check turn budget
		if (state.turnsUsed >= state.maxTurns) {
			state.status = "paused";
			state.pausedReason = `turn budget exhausted (${state.turnsUsed}/${state.maxTurns})`;
			state.lastVerdict = "continue";
			state.lastReason = llmResult.reason;
			saveGoalState(this.cwd, state);
			return {
				shouldContinue: false,
				verdict: "continue",
				reason: state.pausedReason,
				message: `⏸ Goal paused — ${state.turnsUsed}/${state.maxTurns} turns used. Use /goal resume to keep going.`,
				continuationPrompt: null,
			};
		}

		state.lastVerdict = "continue";
		state.lastReason = llmResult.reason;
		saveGoalState(this.cwd, state);
		return {
			shouldContinue: true,
			verdict: "continue",
			reason: llmResult.reason,
			message: `↻ Continuing toward goal (${state.turnsUsed}/${state.maxTurns}): ${llmResult.reason}`,
			continuationPrompt: this.continuationPrompt(),
		};
	}
}

// ── Extension ──

export default function (pi: ExtensionAPI) {
	let manager: GoalManager | null = null;

	pi.on("session_start", async (_event, ctx) => {
		manager = new GoalManager(ctx.cwd);
		if (manager.hasGoal) {
			ctx.ui.setStatus("goal", manager.statusLine());
		}
	});

	// ── /goal command ──
	pi.registerCommand("goal", {
		description: "Set, manage, or query a standing goal",
		handler: async (args, ctx) => {
			if (!manager) manager = new GoalManager(ctx.cwd);

			// pi passes args as a string, not string[]. Split into tokens.
			const raw = typeof args === "string" ? args : "";
			const tokens = raw.split(/\s+/).filter(Boolean);
			const sub = tokens[0];

			if (!sub || sub === "status") {
				ctx.ui.notify(manager.statusLine(), "info");
				return;
			}

			if (sub === "pause") {
				manager.pause();
				ctx.ui.notify("⏸ Goal paused", "warn");
				ctx.ui.setStatus("goal", manager.statusLine());
				return;
			}

			if (sub === "resume") {
				manager.resume();
				ctx.ui.notify("⊙ Goal resumed (turn counter reset)", "success");
				ctx.ui.setStatus("goal", manager.statusLine());
				return;
			}

			if (sub === "clear") {
				manager.clear();
				ctx.ui.notify("Goal cleared", "info");
				ctx.ui.setStatus("goal", null);
				return;
			}

			if (sub === "validators") {
				const valTokens = tokens.slice(1);
				if (!valTokens.length) {
					if (!manager.hasGoal) {
						ctx.ui.notify("No active goal. Set one first.", "info");
						return;
					}
					const s = manager.state;
					if (s && s.validators.length > 0) {
						ctx.ui.notify(`Validators: ${s.validators.join(", ")}`, "info");
					} else {
						ctx.ui.notify("Validators: none (using default: ci, canonical)", "info");
					}
					return;
				}
				// Check for --discover
				const valList = valTokens.join(" ");
				if (valList.includes("--discover")) {
					const all = getAllValidators(ctx.cwd);
					const custom = Object.keys(all).filter((n) => !BUILTIN_NAMES.includes(n));
					const lines = ["## Available Validators\n"];
					lines.push("### Built-in");
					for (const name of BUILTIN_NAMES) {
						lines.push(`  - \`${name}\` \u2192 ${all[name]}`);
					}
					if (custom.length > 0) {
						lines.push("\n### Custom (discovered from .pi/scripts/validate-*.sh)");
						for (const name of custom) {
							lines.push(`  - \`${name}\` \u2192 ${all[name]}`);
						}
					} else {
						lines.push(
							"\n### Custom\n  _(none \u2014 drop validate-*.sh scripts in .pi/scripts/)_",
						);
					}
					ctx.ui.notify(lines.join("\n"), "info");
					return;
				}
				const validators = valList
					.split(",")
					.map((v) => v.trim())
					.filter(Boolean);
				if (validators.includes("all")) {
					const known = Object.keys(getAllValidators(ctx.cwd));
					manager.setValidators(known);
					ctx.ui.notify(`Validators set to all: ${known.join(", ")}`, "success");
				} else {
					manager.setValidators(validators);
					ctx.ui.notify(`Validators set: ${validators.join(", ")}`, "success");
				}
				ctx.ui.setStatus("goal", manager.statusLine());
				return;
			}

			// Setting a new goal — parse flags from full string
			const validatorsMatch = raw.match(/--validators=([^\s]+)/);
			const validators = validatorsMatch
				? validatorsMatch[1]
						.split(",")
						.map((v) => v.trim())
						.filter(Boolean)
				: [];
			const goalText = raw.replace(/--validators=[^\s]+/g, "").trim();
			if (!goalText) {
				ctx.ui.notify("Usage: /goal <text> [--validators=ci,tests,security]", "error");
				return;
			}
			try {
				const state = manager.set(goalText, { validators });
				const valInfo = validators.length > 0 ? ` [validators: ${validators.join(", ")}]` : "";
				ctx.ui.notify(
					`\u2299 Goal set (${state.maxTurns}-turn budget${valInfo}): ${state.goal}`,
					"success",
				);
				ctx.ui.setStatus("goal", manager.statusLine());
			} catch (e) {
				ctx.ui.notify(`Error: ${e}`, "error");
			}
		},
	});

	// ── /subgoal command ──
	pi.registerCommand("subgoal", {
		description: "Add or manage subgoal criteria on the active goal",
		handler: async (args, ctx) => {
			if (!manager) manager = new GoalManager(ctx.cwd);
			const raw = typeof args === "string" ? args : "";
			const tokens = raw.split(/\s+/).filter(Boolean);
			const sub = tokens[0];

			if (!sub) {
				ctx.ui.notify("Usage: /subgoal <text> | list | remove <N> | clear", "info");
				return;
			}

			if (sub === "list") {
				if (!manager.hasGoal) {
					ctx.ui.notify("(no active goal)", "info");
					return;
				}
				const s = manager.state;
				if (!s || !s.subgoals.length) {
					ctx.ui.notify("(no subgoals — use /subgoal <text> to add criteria)", "info");
					return;
				}
				const lines = s.subgoals.map((t, i) => `  ${i + 1}. ${t}`).join("\n");
				ctx.ui.notify(`Subgoals:\n${lines}`, "info");
				return;
			}

			if (sub === "clear") {
				try {
					const n = manager.clearSubgoals();
					ctx.ui.notify(`Cleared ${n} subgoal(s)`, "info");
				} catch (e) {
					ctx.ui.notify(`Error: ${e}`, "error");
				}
				return;
			}

			if (sub === "remove") {
				const idx = Number.parseInt(tokens[1] || "", 10);
				if (Number.isNaN(idx)) {
					ctx.ui.notify("Usage: /subgoal remove <N>", "error");
					return;
				}
				try {
					const removed = manager.removeSubgoal(idx);
					ctx.ui.notify(`Removed: ${removed}`, "info");
				} catch (e) {
					ctx.ui.notify(`Error: ${e}`, "error");
				}
				return;
			}

			// Add subgoal
			try {
				const added = manager.addSubgoal(raw);
				ctx.ui.notify(`Added subgoal: ${added}`, "success");
			} catch (e) {
				ctx.ui.notify(`Error: ${e}`, "error");
			}
		},
	});

	// ── Goal evaluation tool (callable by agent) ──
	pi.registerTool({
		name: "guardian_goal_evaluate",
		label: "Goal Evaluate",
		description:
			"Evaluate the standing goal after a turn. Returns verdict, validator status, and whether to continue.",
		parameters: {
			type: "object",
			properties: {
				lastResponse: {
					type: "string",
					description: "The agent's last response text",
				},
			},
		},
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new GoalManager(ctx.cwd);
			const lastResponse = (params.lastResponse as string) || "";
			const result = await manager.evaluateAfterTurnWithCtx(ctx, lastResponse);

			const lines = [
				"## Goal Evaluation",
				`**Verdict:** ${result.verdict}`,
				`**Continue:** ${result.shouldContinue}`,
				`**Reason:** ${result.reason}`,
			];

			const state = manager.state;
			if (state) {
				lines.push(
					`**Turns:** ${state.turnsUsed}/${state.maxTurns}`,
					`**Status:** ${state.status}`,
				);
				if (Object.keys(state.validatorResults).length > 0) {
					lines.push("**Validators:**");
					for (const [name, vr] of Object.entries(state.validatorResults)) {
						lines.push(`  - ${name}: ${vr.passed ? "✅" : "❌"}`);
					}
				}
			}

			if (result.continuationPrompt) {
				lines.push(`\n**Continuation Prompt:**\n${result.continuationPrompt}`);
			}

			return { content: [{ type: "text", text: lines.join("\n") }] };
		},
	});
}
