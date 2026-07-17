/**
 * Architect Extension — Full Architecture-to-Implementation Pipeline
 *
 * Entry point. Imports from submodules and registers the extension.
 */

import { existsSync, mkdirSync, readFileSync, readdirSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import type { ArchitectureSlice, EpicState, ExtensionAPI, ExtensionContext, ModuleComponent, RoadmapPhase } from "./architect-lib/types.ts";
import {
	ARCH_MODULES_DIR,
	commandExists,
	createRemoteIssue,
	discoverModules,
	ensureRemoteRepo,
	findModuleByName,
	findNextLogicalSlice,
	formatRoadmapStatus,
	getGitBaseUrl,
	getNextPendingPhase,
	linkRemoteIssue,
	loadRoadmapState,
	parseModuleFile,
	parseRoadmap,
	readRepository,
	readLanguage,
	readRepoTool,
	runScript,
	saveRoadmapState,
	createGitlabEpic,
} from "./architect-lib/helpers";
import {
	generateArchitectureReadinessMarkdown,
	generateContractFreezeMarkdown,
	generateIssueMarkdown,
	generateProofingMarkdown,
} from "./architect-lib/generators";
import {
	generateEpicTestFiles,
	isTddSupported,
} from "./architect-lib/tdd-generator";

// ── Helpers ──

/**
 * Parse CLI-style arguments with quote awareness.
 * Handles double-quoted strings so that `--epic "My Epic"` yields ["--epic", "My Epic"].
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

// ── Epic State Persistence ──

const EPIC_STATE_KEY = ".pi/.guardian-epic-state.json";

function loadEpicState(cwd: string): EpicState | null {
	const p = join(cwd, EPIC_STATE_KEY);
	try {
		if (!existsSync(p)) return null;
		return JSON.parse(readFileSync(p, "utf-8")) as EpicState;
	} catch {
		return null;
	}
}

function saveEpicState(cwd: string, state: EpicState): void {
	const p = join(cwd, EPIC_STATE_KEY);
	const dir = dirname(p);
	if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
	writeFileSync(p, JSON.stringify(state, null, 2), "utf-8");
}

function formatEpicStatus(state: EpicState | null): string {
	if (!state) return "No active epic";
	const slice = state.slices?.[0];
	if (!slice) return `Epic "${state.name}" — no slices`;
	const components = slice.nextLogicalSlice || [];
	const done = components.filter((c: ModuleComponent) => c.status === "implemented").length;
	const total = components.length;
	return [
		`Epic: ${state.name}`,
		`Module: ${slice.module}`,
		`Progress: ${done}/${total} components`,
		`Issues: ${(state.issues || []).length}`,
		`Pipeline: ${state.status}`,
		state.tdd ? `TDD: enabled` : `TDD: disabled`,
	].join("\n");
}

// ── Epic Manager ──

class EpicManager {
	private state: EpicState | null;

	constructor(private cwd: string) {
		this.state = loadEpicState(cwd);
	}

	getState(): EpicState | null {
		return this.state;
	}

	async startEpic(
		ctx: ExtensionContext,
		name: string,
		trackingIssueId?: string,
		tdd = false,
	): Promise<EpicState> {
		const moduleFiles = discoverModules(this.cwd);
		if (moduleFiles.length === 0) {
			throw new Error("No architecture modules found in .pi/architecture/modules/.");
		}

		// Try to match epic name to a module doc
		const matchedModule = findModuleByName(this.cwd, name);
		let slice: ArchitectureSlice | null = null;
		if (matchedModule) {
			const components = parseModuleFile(join(this.cwd, ARCH_MODULES_DIR, matchedModule));
			const planned = components.filter((c: ModuleComponent) => c.status === "planned");
			if (planned.length > 0) {
				slice = { module: matchedModule.replace(".md", ""), components, nextLogicalSlice: planned };
			}
		}
		// Fallback: first module with planned components
		if (!slice) {
			slice = findNextLogicalSlice(this.cwd, moduleFiles);
		}
		if (!slice) {
			throw new Error("All architecture components are implemented. No next slice found.");
		}

		ctx.ui.setStatus("architect", `Planning epic: ${name}`);

		// Ensure git repo is initialized before any remote operations
		const gitCheck = runScript(this.cwd, "git rev-parse --git-dir 2>/dev/null");
		if (gitCheck.exitCode !== 0) {
			runScript(this.cwd, "git init");
			runScript(this.cwd, "git add .");
			runScript(this.cwd, 'git commit -m "Initial Guardian scaffold"');
		}

		const repoTool = readRepoTool(this.cwd);
		const repository = readRepository(this.cwd);
		const targetRepo = repository || slice.module;
		let hasRemote = false;
		let remoteRepo = "";

		if (repoTool === "glab" ? commandExists("glab") : commandExists("gh")) {
			const authCheck = runScript(
				this.cwd,
				repoTool === "glab" ? "glab auth status 2>/dev/null" : "gh auth status 2>/dev/null",
			);
			if (authCheck.exitCode === 0) {
				// Try to ensure remote repo — ensureRemoteRepo handles both:
				// - existing repos: configures local origin
				// - nonexistent repos: creates remote repo via gh/glab CLI
				remoteRepo = ensureRemoteRepo(this.cwd, targetRepo, name, repoTool);
				hasRemote = remoteRepo.length > 0;
			}
		}

		// Abort if remote setup was required but failed
		// (repoTool=glab or gh means the user explicitly chose GitHub/GitLab)
		if (!hasRemote && repoTool !== "local") {
			console.error(
				`✖ Remote setup failed for "${targetRepo}" (repoTool: "${repoTool}").`
				+ "\n\n  To fix:"
				+ "\n  1. Ensure glab/gh CLI is authenticated (glab auth status / gh auth status)"
				+ `\n  2. Verify "${targetRepo}" is a valid repository path`
				+ "\n  3. Or set repoTool: \"local\" in guardian-manifest.json for local-only mode"
				+ "\n",
			);
			ctx.ui.notify("Epic aborted — remote setup failed. Set repoTool: \"local\" to run without a remote forge.", "error");
			return null as unknown as EpicState;
		}

		const issues: { id: string; title: string; status: string; remoteIssueId?: string | null }[] = [];
		const issuesDir = join(this.cwd, ".pi/issues");
		if (!existsSync(issuesDir)) mkdirSync(issuesDir, { recursive: true });

		// 0. Auto-create tracking issue (unless user provided one)
		let effectiveTrackingId = trackingIssueId || null;
		if (!effectiveTrackingId && hasRemote && remoteRepo) {
			const trackingBody = [
				`# Epic: ${name}`,
				"",
				`**Module:** ${slice.module}`,
				`**Created:** ${new Date().toISOString()}`,
				"",
				"## Components",
				...slice.nextLogicalSlice.map((c: ModuleComponent) => `- ${c.name}: ${c.description.slice(0, 120)}`),
				"",
				"## Issues",
				"| # | Issue | Status |",
				"|---|-------|--------|",
				"| 1 | Contract Freeze | planned |",
				...slice.nextLogicalSlice.map((c: ModuleComponent, i: number) =>
					`| ${i + 2} | ${c.name} | planned |`,
				),
				`| ${slice.nextLogicalSlice.length + 2} | Proofing & CI | planned |`,
				`| ${slice.nextLogicalSlice.length + 3} | Architecture Readiness | planned |`,
				"",
				"## Pipeline",
				"Steps: implement → validate → create-mr → merge",
				"",
				"---",
				"Auto-generated by Guardian Architect",
			].join("\n");
			const trackingBodyFile = join(issuesDir, ".tracking-issue-body.md");
			writeFileSync(trackingBodyFile, trackingBody);
			const trackingResult = createRemoteIssue(
				this.cwd,
				`Epic: ${name}`,
				trackingBodyFile,
				"epic,tracking",
				remoteRepo,
			);
			if (trackingResult.success && trackingResult.issueNumber) {
				effectiveTrackingId = trackingResult.issueNumber;
			}
			try { if (existsSync(trackingBodyFile)) unlinkSync(trackingBodyFile); } catch { /* ignore */ }
		}

		// 1. Contract freeze
		const freezeId = "issue-contract-freeze";
		const freezeEntry = {
			id: freezeId,
			title: "Contract Freeze: Define interfaces and contracts",
			status: "planned",
			remoteIssueId: null as string | null,
		};
		const freezeMarkdown = generateContractFreezeMarkdown(slice, name, undefined, this.cwd);
		writeFileSync(join(issuesDir, `${freezeId}.md`), freezeMarkdown);
		if (hasRemote && remoteRepo) {
			const result = createRemoteIssue(this.cwd, freezeEntry.title, join(issuesDir, `${freezeId}.md`), "epic,contract", remoteRepo);
			if (result.success && result.issueNumber) {
				freezeEntry.remoteIssueId = result.issueNumber;
				if (effectiveTrackingId) linkRemoteIssue(this.cwd, result.issueNumber, effectiveTrackingId);
			}
		}
		issues.push(freezeEntry);

		// 2. Implementation issues
		for (let i = 0; i < slice.nextLogicalSlice.length; i++) {
			const comp = slice.nextLogicalSlice[i];
			const moduleId = slice.module.replace(/^module-/, "");
			const id = `issue-${comp.name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
			const entry = {
				id,
				title: `Implement ${comp.name} — ${moduleId}`,
				status: "planned" as string,
				remoteIssueId: null as string | null,
			};
			const md = generateIssueMarkdown(comp, slice, i, slice.nextLogicalSlice.length, tdd, this.cwd);
			writeFileSync(join(issuesDir, `${id}.md`), md);
			if (hasRemote && remoteRepo) {
				const result = createRemoteIssue(this.cwd, entry.title, join(issuesDir, `${id}.md`), "epic,implementation", remoteRepo);
				if (result.success && result.issueNumber) {
					entry.remoteIssueId = result.issueNumber;
					if (effectiveTrackingId) linkRemoteIssue(this.cwd, result.issueNumber, effectiveTrackingId);
				}
			}
			issues.push(entry);
		}

		// 3. Proofing
		const proofingId = "issue-proofing";
		const proofingEntry = {
			id: proofingId,
			title: "Proofing: Validation scripts + CI integration",
			status: "planned" as string,
			remoteIssueId: null as string | null,
		};
		const proofingMd = generateProofingMarkdown(slice, name);
		writeFileSync(join(issuesDir, `${proofingId}.md`), proofingMd);
		if (hasRemote && remoteRepo) {
			const result = createRemoteIssue(this.cwd, proofingEntry.title, join(issuesDir, `${proofingId}.md`), "epic,proofing", remoteRepo);
			if (result.success && result.issueNumber) {
				proofingEntry.remoteIssueId = result.issueNumber;
				if (effectiveTrackingId) linkRemoteIssue(this.cwd, result.issueNumber, effectiveTrackingId);
			}
		}
		issues.push(proofingEntry);

		// 4. Architecture readiness
		const readinessId = "issue-architecture-readiness";
		const readinessEntry = {
			id: readinessId,
			title: "Architecture Readiness: Runbook, DR, docs, CI enforcement",
			status: "planned" as string,
			remoteIssueId: null as string | null,
		};
		const readinessMd = generateArchitectureReadinessMarkdown(slice, name);
		writeFileSync(join(issuesDir, `${readinessId}.md`), readinessMd);
		if (hasRemote && remoteRepo) {
			const result = createRemoteIssue(this.cwd, readinessEntry.title, join(issuesDir, `${readinessId}.md`), "epic,architecture-readiness", remoteRepo);
			if (result.success && result.issueNumber) {
				readinessEntry.remoteIssueId = result.issueNumber;
				if (effectiveTrackingId) linkRemoteIssue(this.cwd, result.issueNumber, effectiveTrackingId);
			}
		}
		issues.push(readinessEntry);

		// ── TDD test generation ──
		const tddTestFiles: string[] = [];
		if (tdd) {
			const language = readLanguage(this.cwd);
			if (isTddSupported(language)) {
				tddTestFiles.push(...generateEpicTestFiles({
					components: slice.nextLogicalSlice,
					moduleId: slice.module,
					cwd: this.cwd,
					language,
				}));
			}
		}

		// ── GitLab Epic creation (only for GitLab repos with remote access) ──
		let effectiveEpicId: string | null = null;
		if (repoTool === "glab" && hasRemote && remoteRepo) {
			const epicTitle = `Epic: ${name} — ${slice.module}`;
			const epicDesc = [
				`# ${epicTitle}`,
				"",
				`**Module:** ${slice.module}`,
				`**Components:** ${slice.nextLogicalSlice.length}`,
				"",
				"## Issues",
				...issues.map((i) => `- ${i.title}${i.remoteIssueId ? ` (#${i.remoteIssueId})` : ""}`),
				"",
				"---",
				"Auto-generated by Guardian Architect",
			].join("\n");
			effectiveEpicId = createGitlabEpic(this.cwd, remoteRepo, epicTitle, epicDesc);
			if (effectiveEpicId) {
				console.warn(`Created GitLab epic #${effectiveEpicId}`);
			}
		}

		const state: EpicState = {
			name,
			trackingIssueId: effectiveTrackingId,
			epicId: effectiveEpicId,
			slices: [slice],
			issues,
			status: "planning",
			currentIssueIndex: 0,
			createdAt: new Date().toISOString(),
			tdd,
			tddTestFiles,
		};

		this.state = state;
		saveEpicState(this.cwd, state);
		return state;
	}

	async abortEpic(): Promise<void> {
		this.state = null;
		try {
			const p = join(this.cwd, ".pi/.guardian-epic-state.json");
			if (existsSync(p)) unlinkSync(p);
		} catch { /* ignore */ }
	}
}

// ── Extension ──

export default function (pi: ExtensionAPI) {
	let manager: EpicManager | null = null;

	function findFlag(tokens: string[], prefix: string): string | undefined {
		const eqMatch = tokens.find((a) => a.startsWith(`${prefix}=`));
		if (eqMatch) return eqMatch.split("=").slice(1).join("=");
		const idx = tokens.indexOf(prefix);
		if (idx >= 0 && idx + 1 < tokens.length && !tokens[idx + 1].startsWith("--")) return tokens[idx + 1];
		return undefined;
	}

	pi.registerCommand("architect", {
		description: "Orchestrate the full architecture-to-implementation process",
		handler: async (args, ctx) => {
			if (!manager) manager = new EpicManager(ctx.cwd);
			const raw = typeof args === "string" ? args : "";
			const tokens = raw ? parseArgs(raw) : [];
			if (tokens.length === 0) {
				ctx.ui.notify(
					"Usage: /architect [--epic Name] [--tracking-issue N] [--tdd] | --roadmap | --phase \"Phase 1\" | --phase-status | --phase-done <N> | --phase-module-done <N> \"Module\" | status | next-epic | abort",
					"info",
				);
				return;
			}
			const action = tokens[0];

			if (action === "status" || action === "") {
				const state = manager.getState();
				ctx.ui.notify(formatEpicStatus(state), "info");
				return;
			}

			if (action === "abort") {
				await manager.abortEpic();
				ctx.ui.notify("Epic aborted", "error");
				return;
			}

			if (action === "next-epic") {
				const moduleFiles = discoverModules(ctx.cwd);
				const slice = findNextLogicalSlice(ctx.cwd, moduleFiles);
				if (!slice) {
					ctx.ui.notify("No more architecture slices to implement.", "info");
					return;
				}
				ctx.ui.notify(`Next epic: ${slice.module} (${slice.nextLogicalSlice.length} components planned)`, "info");
				return;
			}

			// ── Roadmap commands ──
			if (tokens[0] === "--roadmap" || action === "roadmap") {
				const phases = parseRoadmap(ctx.cwd);
				ctx.ui.notify(formatRoadmapStatus(phases), "info");
				return;
			}

			if (tokens[0] === "--phase-status" || action === "phase-status") {
				const phases = parseRoadmap(ctx.cwd);
				const next = getNextPendingPhase(phases);
				if (!next) {
					const allDone = phases.every((p) => p.status === "done");
					ctx.ui.notify(allDone ? "All phases complete! 🎉" : "Next phase blocked by dependencies.", "info");
					return;
				}
				ctx.ui.notify(`Next phase: Phase ${next.index}: ${next.title} (${next.modules.length} modules)`, "info");
				return;
			}

			if (tokens[0] === "--phase" || action === "phase") {
				const phaseName = (tokens.slice(1).join(" ") || findFlag(tokens, "--phase")).replace(/["']/g, "").trim();
				if (!phaseName) {
					ctx.ui.notify('Usage: /architect --phase "Phase 1"', "error");
					return;
				}

				const phases = parseRoadmap(ctx.cwd);
				if (phases.length === 0) {
					ctx.ui.notify("No implementation-roadmap.md found in .pi/architecture/.", "error");
					return;
				}

				// Find phase by name or index
				let targetPhase: RoadmapPhase | undefined;
				const idxMatch = phaseName.match(/Phase\s+(\d+)/i);
				if (idxMatch) {
					targetPhase = phases.find((p) => p.index === parseInt(idxMatch[1], 10));
				} else {
					targetPhase = phases.find((p) => p.title.toLowerCase().includes(phaseName.toLowerCase()));
				}

				if (!targetPhase) {
					ctx.ui.notify(`Phase "${phaseName}" not found in roadmap.`, "error");
					return;
				}

				// Check dependencies
				const unmetDeps = targetPhase.dependencies.filter((dep) => {
					if (dep.toLowerCase() === "none") return false;
					const depMatch = dep.match(/Phase\s+(\d+)/i);
					if (!depMatch) return false;
					const depPhase = phases.find((p) => p.index === parseInt(depMatch[1], 10));
					return depPhase && depPhase.status !== "done";
				});
				if (unmetDeps.length > 0) {
					ctx.ui.notify(
						`Cannot start Phase ${targetPhase.index}: unmet dependencies (${unmetDeps.join(", ")}).`,
						"error",
					);
					return;
				}

				if (targetPhase.status === "done") {
					ctx.ui.notify(`Phase ${targetPhase.index}: ${targetPhase.title} is already complete.`, "info");
					return;
				}

				// Mark phase as in_progress
				const roadmapState = loadRoadmapState(ctx.cwd) || {
					phases: [],
					currentPhaseIndex: 0,
					startedAt: new Date().toISOString(),
					updatedAt: new Date().toISOString(),
				};
				roadmapState.phases = phases.map((p) => ({
					...p,
					status: p.index === targetPhase!.index ? "in_progress" : p.status,
				}));
				roadmapState.currentPhaseIndex = targetPhase.index;
				roadmapState.updatedAt = new Date().toISOString();
				saveRoadmapState(ctx.cwd, roadmapState);

				// Create standard epic for each module using the same pipeline as --epic
				const results: string[] = [];
				for (const mod of targetPhase.modules) {
					if (targetPhase.completedModules?.includes(mod.name)) {
						results.push(`✅ ${mod.name} — already completed, skipped`);
						continue;
					}

					// Check if issues already exist for this module (session restart)
					const issuesDir = join(ctx.cwd, ".pi/issues");
					const freezePath = join(issuesDir, "issue-contract-freeze.md");
					if (existsSync(freezePath)) {
						// Issues exist — reconstruct pipeline without re-creating
						const existingIssues: string[] = [];
						if (existsSync(issuesDir)) {
							const files = readdirSync(issuesDir);
							for (const f of files) {
								if (f.endsWith(".md") && !f.startsWith(".")) {
									existingIssues.push(f.replace(/\.md$/, ""));
								}
							}
						}
						const pipelineFile = join(ctx.cwd, ".pi/.guardian-pipeline-state.json");
						if (!existsSync(pipelineFile) && existingIssues.length > 0) {
							const pipelineId = `PL-${String(Math.floor(Math.random() * 10000)).padStart(4, "0")}`;
							const pipelineState = {
								id: pipelineId,
								name: mod.name,
								items: existingIssues,
								steps: [
									{ name: "implement", prompt: ".pi/prompts/issue-implementation-series.md", acceptance: { type: "validator", validators: ["ci"] } },
									{ name: "validate", acceptance: { type: "validator", validators: ["ci", "tests", "security"] } },
									{ name: "create-mr", prompt: ".pi/prompts/issue-closeout.md", acceptance: { type: "none" } },
									{ name: "merge", prompt: ".pi/prompts/issue-merge.md", acceptance: { type: "validator", validators: ["ci", "canonical"] } },
								],
								currentItemIndex: 0,
								currentStepIndex: 0,
								status: "running",
								retryCount: 0,
								results: [],
								mergeOnValid: true,
								createdAt: new Date().toISOString(),
								updatedAt: new Date().toISOString(),
							};
							const pipelineDir = dirname(pipelineFile);
							if (!existsSync(pipelineDir)) mkdirSync(pipelineDir, { recursive: true });
							writeFileSync(pipelineFile, JSON.stringify(pipelineState, null, 2));
						}
						results.push(`📋 ${mod.name} — ${existingIssues.length} issues found (pipeline resumed)`);
						continue;
					}

					// Check if module has planned components BEFORE calling startEpic
					// (startEpic silently falls back to wrong module if no planned components)
					const matchedFile = findModuleByName(ctx.cwd, mod.name);
					let hasPlanned = false;
					if (matchedFile) {
						const comps = parseModuleFile(join(ctx.cwd, ARCH_MODULES_DIR, matchedFile));
						hasPlanned = comps.some((c) => c.status === "planned");
					}
					if (!hasPlanned) {
						results.push(`✅ ${mod.name} — all components implemented, skipped`);
						// Auto-mark as completed module
						const rs = loadRoadmapState(ctx.cwd) || {
							phases: [], currentPhaseIndex: 0, startedAt: "", updatedAt: "",
						};
						const comp = new Set((rs.phases.find((p) => p.index === targetPhase.index)?.completedModules || []));
						comp.add(mod.name);
						if (rs.phases.find((p) => p.index === targetPhase.index)) {
							rs.phases.find((p) => p.index === targetPhase.index)!.completedModules = Array.from(comp);
						}
						rs.updatedAt = new Date().toISOString();
						saveRoadmapState(ctx.cwd, rs);
						continue;
					}
					try {
						const state = await manager.startEpic(ctx, mod.name);
						if (!state || !state.slices || state.slices.length === 0) {
							results.push(`⚠️ ${mod.name} — no architecture components found`);
							continue;
						}
						const items = (state.issues || []).map((i: { id: string }) => i.id);
						// Create pipeline state for this epic
						const pipelineId = `PL-${String(Math.floor(Math.random() * 10000)).padStart(4, "0")}`;
						const pipelineState = {
							id: pipelineId,
							name: mod.name,
							items,
							steps: [
								{ name: "implement", prompt: ".pi/prompts/issue-implementation-series.md", acceptance: { type: "validator", validators: ["ci"] } },
								{ name: "validate", acceptance: { type: "validator", validators: ["ci", "tests", "security"] } },
								{ name: "create-mr", prompt: ".pi/prompts/issue-closeout.md", acceptance: { type: "none" } },
								{ name: "merge", prompt: ".pi/prompts/issue-merge.md", acceptance: { type: "validator", validators: ["ci", "canonical"] } },
							],
							currentItemIndex: 0,
							currentStepIndex: 0,
							status: "running",
							retryCount: 0,
							results: [],
							mergeOnValid: true,
							createdAt: new Date().toISOString(),
							updatedAt: new Date().toISOString(),
						};
						// Write pipeline state only for the FIRST module (active pipeline)
						// Other epics' pipelines are tracked in the phase state, not active
						const pipelineFile = join(ctx.cwd, ".pi/.guardian-pipeline-state.json");
						if (!existsSync(pipelineFile)) {
							const pipelineDir = dirname(pipelineFile);
							if (!existsSync(pipelineDir)) mkdirSync(pipelineDir, { recursive: true });
							writeFileSync(pipelineFile, JSON.stringify(pipelineState, null, 2));
						}
						results.push(`📋 ${mod.name} — ${items.length} issues created (pipeline ${pipelineId})`);
					} catch (e) {
						results.push(`❌ ${mod.name} — error: ${e}`);
					}
				}

				// Mark completed modules from roadmap state
				const updatedRoadmap = loadRoadmapState(ctx.cwd) || roadmapState;
				updatedRoadmap.updatedAt = new Date().toISOString();
				saveRoadmapState(ctx.cwd, updatedRoadmap);

				// Find the first module that got a pipeline (active epic)
				const firstActive = results.find((r) => r.startsWith("📋"));
				const activeEpic = firstActive ? firstActive.replace(/^📋\s*/, "").split(" —")[0] : null;

				const summary = [
					`## Phase ${targetPhase.index}: ${targetPhase.title} — Epics Created`,
					`**Goal:** ${targetPhase.goal}`,
					`**Days:** ${targetPhase.days}`,
					"",
					"### Results",
					...results.map((r) => `- ${r}`),
					"",
					"### How to implement",
					activeEpic ? `Active epic: **${activeEpic}** — use \`pipeline_next_task\` to start.` : "All modules already implemented.",
					"When an epic is done, use /architect --epic \"<next-module>\" to start the next one.",
					"",
					"After completing all module epics, close the phase:",
					`  \`/architect --phase-done ${targetPhase.index}\``,
					"",
					"### Acceptance Criteria",
					targetPhase.criteria.map((c) => `- [ ] ${c}`).join("\n"),
				].join("\n");

				ctx.ui.notify(
					`Phase ${targetPhase.index}: ${targetPhase.title} — ${results.length} modules processed`,
					"success",
				);
				pi.sendMessage(
					{ content: summary, display: true },
					{ deliverAs: "followUp", triggerTurn: true },
				);
				return;
			}

			if (tokens[0] === "--phase-module-done" || action === "phase-module-done") {
				const phaseIdx = parseInt(tokens[1] || "", 10);
				if (isNaN(phaseIdx) || !tokens[2]) {
					ctx.ui.notify('Usage: /architect --phase-module-done <phase-number> "<module-name>"', "error");
					return;
				}
				const rawName = tokens.slice(2).join(" ").replace(/["']/g, "").trim();
				const phases = parseRoadmap(ctx.cwd);
				const phase = phases.find((p) => p.index === phaseIdx);
				if (!phase) {
					ctx.ui.notify(`Phase ${phaseIdx} not found.`, "error");
					return;
				}
				const matchedModule = phase.modules.find(
					(m) => m.name.toLowerCase() === rawName.toLowerCase(),
				);
				if (!matchedModule) {
					ctx.ui.notify(
						`Module "${rawName}" not found in Phase ${phaseIdx}. Available: ${phase.modules.map((m) => m.name).join(", ")}`,
						"error",
					);
					return;
				}
				const roadmapState = loadRoadmapState(ctx.cwd) || {
					phases: [],
					currentPhaseIndex: 0,
					startedAt: new Date().toISOString(),
					updatedAt: new Date().toISOString(),
				};
				const completed = new Set(phase.completedModules || []);
				completed.add(matchedModule.name);
				roadmapState.phases = phases.map((p) => ({
					...p,
					completedModules: p.index === phaseIdx ? Array.from(completed) : p.completedModules,
				}));
				roadmapState.updatedAt = new Date().toISOString();
				saveRoadmapState(ctx.cwd, roadmapState);
				const done = completed.size;
				const total = phase.modules.length;
				ctx.ui.notify(`Phase ${phaseIdx}: "${matchedModule.name}" marked done (${done}/${total} modules)`, "success");
				return;
			}

			if (tokens[0] === "--phase-done" || action === "phase-done") {
				const phaseIdx = parseInt(tokens[1] || "", 10);
				if (isNaN(phaseIdx)) {
					ctx.ui.notify('Usage: /architect --phase-done <phase-number>', "error");
					return;
				}
				const phases = parseRoadmap(ctx.cwd);
				const phase = phases.find((p) => p.index === phaseIdx);
				if (!phase) {
					ctx.ui.notify(`Phase ${phaseIdx} not found.`, "error");
					return;
				}
				const roadmapState = loadRoadmapState(ctx.cwd) || {
					phases: [],
					currentPhaseIndex: 0,
					startedAt: new Date().toISOString(),
					updatedAt: new Date().toISOString(),
				};
				roadmapState.phases = phases.map((p) => ({
					...p,
					status: p.index === phaseIdx ? "done" : p.status,
				}));
				roadmapState.updatedAt = new Date().toISOString();
				saveRoadmapState(ctx.cwd, roadmapState);
				ctx.ui.notify(`Phase ${phaseIdx}: ${phase.title} marked complete! ✅`, "success");

				const next = getNextPendingPhase(
					phases.map((p) => ({ ...p, status: p.index === phaseIdx ? "done" : p.status })),
				);
				if (next) {
					pi.sendMessage(
						{
							content: `**Next up:** Phase ${next.index}: ${next.title} — run \`/architect --phase "Phase ${next.index}"\` to start.`,
							display: true,
						},
						{ deliverAs: "followUp", triggerTurn: true },
					);
				} else {
					const allDone = phases.every((p) => p.index === phaseIdx || p.status === "done");
					if (allDone) {
						pi.sendMessage(
							{ content: "🎉 **All roadmap phases complete!**", display: true },
							{ deliverAs: "followUp", triggerTurn: true },
						);
					}
				}
				return;
			}

			const epicName = findFlag(tokens, "--epic");
			const trackingIssueId = findFlag(tokens, "--tracking-issue");
			const tddEnabled = tokens.includes("--tdd");

			if (!epicName) {
				ctx.ui.notify('Usage: /architect --epic "Epic Name" [--tracking-issue N] [--tdd]', "error");
				return;
			}

			try {
				if (!epicName || epicName.trim() === "") {
					ctx.ui.notify('Usage: /architect --epic "Epic Name" [--tracking-issue N] [--tdd]', "error");
					return;
				}

				const state = await manager.startEpic(ctx, epicName, trackingIssueId, tddEnabled);

				if (!state || !state.slices || state.slices.length === 0) {
					ctx.ui.notify("Failed to discover architecture components. Check .pi/architecture/modules/.", "error");
					return;
				}

				const slice = state.slices[0];
				const components = slice.nextLogicalSlice || [];

				if (components.length === 0) {
					ctx.ui.notify("No planned components found in architecture module.", "error");
					return;
				}

				const items = (state.issues || []).map((i) => i.id);
				if (items.length === 0) {
					ctx.ui.notify("Failed to generate issues.", "error");
					return;
				}



				// Remove stale pipeline state so the new one takes effect
				try {
					const oldPipelinePath = join(ctx.cwd, ".pi/.guardian-pipeline-state.json");
					if (existsSync(oldPipelinePath)) unlinkSync(oldPipelinePath);
				} catch { /* ignore */ }

				// Write pipeline state directly (ctx.tools not available in command handlers)
				const pipelineId = `PL-${String(Math.floor(Math.random() * 10000)).padStart(4, "0")}`;
				const pipelineState = {
					id: pipelineId,
					name: epicName,
					items,
					steps: [
						{ name: "implement", prompt: ".pi/prompts/issue-implementation-series.md", acceptance: { type: "validator", validators: ["ci"] } },
						{ name: "validate", acceptance: { type: "validator", validators: ["ci", "tests", "security"] } },
						{ name: "create-mr", prompt: ".pi/prompts/issue-closeout.md", acceptance: { type: "none" } },
						{ name: "merge", prompt: ".pi/prompts/issue-merge.md", acceptance: { type: "validator", validators: ["ci", "canonical"] } },
					],
					currentItemIndex: 0,
					currentStepIndex: 0,
					status: "running",
					retryCount: 0,
					results: [],
					mergeOnValid: true,
					createdAt: new Date().toISOString(),
					updatedAt: new Date().toISOString(),
				};
				const pipelineDir = dirname(join(ctx.cwd, ".pi/.guardian-pipeline-state.json"));
				if (!existsSync(pipelineDir)) mkdirSync(pipelineDir, { recursive: true });
				writeFileSync(join(ctx.cwd, ".pi/.guardian-pipeline-state.json"), JSON.stringify(pipelineState, null, 2));

				const repository = readRepository(ctx.cwd) || "";
				const baseUrl = getGitBaseUrl(readRepoTool(ctx.cwd));
				const trackingUrl = state.trackingIssueId && repository
					? `\n**Tracking issue:** ${baseUrl}/${repository}/issues/${state.trackingIssueId}`
					: "";

				const firstItem = items[0];
				const issueFilename = `${firstItem}.md`.replace(/\//g, "-");
				const issuePath = join(ctx.cwd, ".pi/issues", issueFilename);

				let issueContent = "";
				try {
					if (existsSync(issuePath)) {
						issueContent = readFileSync(issuePath, "utf-8").replace(/^---[\s\S]*?---\n/, "").trim();
					}
				} catch { /* ignore */ }

				const instructions = [
					`Epic "${epicName}" started with ${items.length} issues across ${components.length} components.${trackingUrl}`,
					"",
					`Pipeline \`${pipelineId}\` created: ${items.length} items × 4 steps (implement → validate → create-mr → merge)`,
					`**Current:** Item "${firstItem}" → Step: implement`,
					"",
					"**Available pipeline tools:**",
					"- `pipeline_next_task` — get full context for current item+step",
					"- `pipeline_run_acceptance` — run validators for current step",
					"- `pipeline_advance` — mark step passed, move to next",
					"- `pipeline_fail` — mark step failed with reason",
					"- `pipeline_status` — check overall progress",
					"",
					"**Workflow per item:**",
					"1. Create branch: `feat/<issue-id>`",
					"2. Implement the component according to the issue context below",
					"3. Run `pipeline_run_acceptance` to validate your work",
					"4. Call `pipeline_advance` to move to the next step",
					"5. Pipeline auto-advances through: implement → validate → create-mr → merge",
					tddEnabled ? [
						"",
						"**TDD Mode: ON**",
						"- Failing test files were generated from architecture contracts before issues were created.",
						"- Start each component by examining its test file in the `tests/unit/` directory.",
						"- Run tests first to see them fail, then implement to make them pass (Red→Green→Refactor).",
						"- Tests are living artifacts — evolve them as the component grows.",
						"- Reference `.pi/skills/tdd-practice.md` for TDD guidance.",
					].join("\n") : "",
					"",
					"---",
					"",
					"## Issue Context",
					"",
					issueContent || `Review .pi/issues/${issueFilename} for full details.`,
				].join("\n");

				pi.sendMessage(
					{ content: instructions, display: true },
					{ deliverAs: "followUp", triggerTurn: true },
				);
				return;
			} catch (e) {
				ctx.ui.notify(`Architect error: ${e}`, "error");
			}
		},
	});

	pi.registerTool({
		name: "architect_status",
		label: "Architect Status",
		description: "Show the current epic status and progress.",
		parameters: { type: "object", properties: {} },
		async execute(_toolCallId, _params, _signal, _onUpdate, ctx) {
			if (!manager) manager = new EpicManager(ctx.cwd);
			const state = manager.getState();
			return { content: [{ type: "text", text: formatEpicStatus(state) }] };
		},
	});

	pi.registerTool({
		name: "architect_discover",
		label: "Architect Discover",
		description: "Discover architecture modules and find the next logical slice.",
		parameters: { type: "object", properties: {} },
		async execute(_toolCallId, _params, _signal, _onUpdate, ctx) {
			const moduleFiles = discoverModules(ctx.cwd);
			if (moduleFiles.length === 0) {
				return { content: [{ type: "text", text: "No architecture modules found in .pi/architecture/modules/." }] };
			}
			const lines = ["## Architecture Modules\n"];
			for (const file of moduleFiles) {
				const components = parseModuleFile(join(ctx.cwd, ".pi/architecture/modules", file));
				const planned = components.filter((c) => c.status === "planned");
				lines.push(`### ${file.replace(".md", "")}`);
				lines.push(`  Components: ${components.length} (${planned.length} planned)`);
				if (planned.length > 0) {
					lines.push("  Next slice:");
					for (const c of planned) lines.push(`    - ${c.name}`);
				}
				lines.push("");
			}
			const slice = findNextLogicalSlice(ctx.cwd, moduleFiles);
			if (slice) {
				lines.push(`**Recommended next epic:** ${slice.module}`);
				lines.push(`Components: ${slice.nextLogicalSlice.map((c: ModuleComponent) => c.name).join(", ")}`);
			}
			return { content: [{ type: "text", text: lines.join("\n") }] };
		},
	});

	pi.registerTool({
		name: "architect_roadmap",
		label: "Architect Roadmap",
		description: "Show the implementation roadmap phases and status.",
		parameters: { type: "object", properties: {} },
		async execute(_toolCallId, _params, _signal, _onUpdate, ctx) {
			const phases = parseRoadmap(ctx.cwd);
			return { content: [{ type: "text", text: formatRoadmapStatus(phases) }] };
		},
	});
}
