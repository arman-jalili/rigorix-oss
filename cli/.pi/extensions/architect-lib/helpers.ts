import { execFileSync, execSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import type { ExtensionContext, EpicState, ModuleComponent, ArchitectureSlice, RoadmapPhase, RoadmapState, RoadmapPhaseModule } from "./types.ts";
import {
	readRepoTool as adapterReadRepoTool,
	readRepository as adapterReadRepository,
	readLanguage as adapterReadLanguage,
	getGitBaseUrl as adapterGetGitBaseUrl,
	commandExists as adapterCommandExists,
	runScript as adapterRunScript,
	verifyForgeAccess,
	glabEncodePath,
	getGlabRemoteUrl,
} from "./forge-adapter.ts";
// ── Constants ──

export const EPIC_STATE_KEY = ".pi/.guardian-epic-state.json";
export const ARCH_MODULES_DIR = ".pi/architecture/modules";
export const ISSUES_DIR = ".pi/issues";
export const ROADMAP_FILE = ".pi/architecture/implementation-roadmap.md";
export const ROADMAP_STATE_FILE = ".pi/.guardian-roadmap-state.json";

// ── Helpers ──

export function log(ctx: ExtensionContext, message: string, level = "info") {
	ctx.ui.notify(message, level);
}

// These functions are now centralized in ../forge-adapter.ts
// We re-export them here for backward compatibility with architect.ts

export const runScript = adapterRunScript;
export const readRepoTool = adapterReadRepoTool;
export const readRepository = adapterReadRepository;
export const readLanguage = adapterReadLanguage;
export const getGitBaseUrl = adapterGetGitBaseUrl;
export const commandExists = adapterCommandExists;

// Try to create a remote GitHub/GitLab issue via the shell script wrapper.
// Uses execFileSync to avoid shell quoting issues with nested commands.
function execSyncWithRetry(
	args: string[],
	cwd: string,
	opts: { timeout?: number; maxRetries?: number; delayMs?: number } = {},
): { exitCode: number; stdout: string } {
	const maxRetries = opts.maxRetries ?? 4;
	const delayMs = opts.delayMs ?? 5000;
	let lastError = "";

	for (let attempt = 0; attempt <= maxRetries; attempt++) {
		if (attempt > 0) {
			const wait = delayMs * Math.pow(1.5, attempt - 1);
			const waitMs = Math.min(wait, 30_000);
			const waitSec = Math.ceil(waitMs / 1000);
			try {
				execFileSync("sleep", [String(waitSec)], { cwd, timeout: 60_000, encoding: "utf-8" });
			} catch { /* ignore */ }
		}
		try {
			const stdout = execFileSync("bash", args, { cwd, timeout: opts.timeout ?? 120_000, encoding: "utf-8" });
			return { exitCode: 0, stdout };
		} catch (e: unknown) {
			const err = e as { status?: number; stdout?: string; message?: string };
			lastError = err.stdout ?? err.message ?? "";
		}
	}
	return { exitCode: 1, stdout: lastError };
}

export function createRemoteIssue(
	cwd: string,
	title: string,
	bodyFilePath: string,
	labels: string,
	repository?: string,
): { success: boolean; issueNumber: string | null; error?: string } {
	const createScript = join(cwd, ".pi/scripts/git/create-tracking-issue.sh");
	if (!existsSync(createScript)) {
		return { success: false, issueNumber: null, error: "create-tracking-issue.sh not found" };
	}

	const args: string[] = [
		createScript,
		"--title",
		title,
		"--body-file",
		bodyFilePath,
		"--labels",
		labels,
	];
	if (repository) args.push("--repo", repository);

	const { exitCode, stdout } = execSyncWithRetry(args, cwd, { maxRetries: 2, delayMs: 2000 });

	if (exitCode !== 0) {
		return { success: false, issueNumber: null, error: stdout };
	}

	const numberMatch = stdout.match(/TRACKING_ID=(\d+)/);
	if (numberMatch) {
		return { success: true, issueNumber: numberMatch[1] };
	}
	const urlMatch = stdout.match(/#(\d+)/);
	if (urlMatch) {
		return { success: true, issueNumber: urlMatch[1] };
	}
	return { success: false, issueNumber: null, error: "Could not parse issue number" };
}

// Create a real GitLab Epic via the glab CLI.
// Returns the epic IID (string) on success, null on failure.
export function createGitlabEpic(
	cwd: string,
	repository: string,
	title: string,
	description: string,
): string | null {
	// Derive group from repository path: "group/subgroup/project" -> "group/subgroup"
	const parts = repository.split("/");
	if (parts.length < 2) return null;
	const group = parts.slice(0, -1).join("/");

	// Build description from markdown content (strip YAML frontmatter)
	const desc = description.replace(/^---[\s\S]*?---\n*/, "").trim();

	// Create epic via glab CLI
	const result = runScript(
		cwd,
		`glab epic create --group "${group}" --title "${title}" --description "${desc.slice(0, 5000)}" 2>&1`,
	);

	if (result.exitCode !== 0) {
		// Fallback: try the API directly (URL-encode nested group paths)
		const encodedGroup = glabEncodePath(group);
		const body = JSON.stringify({ title, description: desc.slice(0, 5000) });
		const apiResult = runScript(
			cwd,
			`glab api --method POST "groups/${encodedGroup}/epics" --body '${body}' 2>/dev/null | jq -r '.iid // empty'`,
		);
		if (apiResult.exitCode !== 0 || !apiResult.stdout.trim()) {
			console.warn(`Failed to create GitLab epic: ${result.stdout || apiResult.stdout}`);
			return null;
		}
		return apiResult.stdout.trim();
	}

	// Parse epic IID from output: "https://gitlab.com/groups/.../-/epics/42"
	const epicMatch = result.stdout.match(/epics\/(\d+)/);
	return epicMatch ? epicMatch[1] : null;
}

// Ensure the GitHub/GitLab repository exists and local git remote is configured.
// Returns the repository slug if remote is ready, empty string if not available.
export function ensureRemoteRepo(
	cwd: string,
	repository: string,
	epicName: string,
	repoTool: string,
): string {
	// Check if remote already exists via git remote
	const remoteCheck = runScript(cwd, "git remote get-url origin 2>/dev/null");
	if (remoteCheck.exitCode === 0) {
		// Validate existing origin: check it's reachable and matches the manifest repo
		const remoteUrl = remoteCheck.stdout.trim();
		const repoSlug = repository.replace(/^https?:\/\//, "").replace(/^git@/, "");

		// Check reachability
		const reachable = runScript(cwd, "git ls-remote origin HEAD 2>/dev/null");
		if (reachable.exitCode !== 0) {
			console.warn(`Origin remote exists but is not reachable (${remoteUrl}). Will re-create.`);
			// Remove and re-create below
			runScript(cwd, "git remote remove origin 2>/dev/null");
		} else {
			// Verify remote URL contains the manifest repository path
			if (remoteUrl.includes(repository.replace(/^https?:\/\//, "").replace(/^git@/, "").replace(/\.git$/, ""))
				|| remoteUrl.includes(repository)) {
				return repository;
			}
			console.warn(`Existing origin points to a different repository. Re-creating.`);
			runScript(cwd, "git remote remove origin 2>/dev/null");
		}
	}

	// Remote not configured locally — ensure the remote repo exists on GitHub/GitLab
	if (repoTool === "gh") {
		const createResult = runScript(
			cwd,
			`gh repo create "${repository}" --private --description "Epic: ${epicName}" 2>&1`,
		);
		if (createResult.exitCode !== 0) {
			console.warn(`Failed to create GitHub repo: ${createResult.stdout}`);
			return "";
		}
		// Remove stale origin if it exists but points nowhere useful
		runScript(cwd, "git remote remove origin 2>/dev/null");
		const httpsUrl = `https://github.com/${repository}.git`;
		runScript(cwd, `git remote add origin "${httpsUrl}"`);
		return repository;
	}

	// GitLab path — detect self-hosted base URL from glab config

	// Split repository path into group + project name for glab CLI
	const parts = repository.split("/");
	const projectName = parts.pop()!;
	const group = parts.join("/");

	const createResult = group
		? runScript(
				cwd,
				`glab repo create "${projectName}" --group "${group}" --private --description "Epic: ${epicName}" 2>&1`,
			)
		: runScript(
				cwd,
				`glab repo create "${repository}" --private --description "Epic: ${epicName}" 2>&1`,
			);

	let projectId = "";
	if (createResult.exitCode !== 0) {
		// Repo may already exist — verify via API before giving up
		const encodedPath = glabEncodePath(repository);
		const viewResult = runScript(
			cwd,
			`glab api "projects/${encodedPath}" --method GET 2>/dev/null | jq -r '.id // empty'`,
		);
		if (viewResult.exitCode !== 0 || !viewResult.stdout.trim()) {
			console.warn(`Failed to create or find GitLab project: ${createResult.stdout}`);
			return "";
		}
		projectId = viewResult.stdout.trim();
		console.warn(`GitLab project already exists (id=${projectId}), connecting to it.`);
	} else {
		// Project was just created — wait for GitLab to provision it
		// (new projects can take 10-30s before accepting issues)
		const encodedPath = glabEncodePath(repository);
		console.warn(`Waiting for GitLab project to be ready...`);
		for (let attempt = 0; attempt < 12; attempt++) {
			const checkResult = runScript(
				cwd,
				`glab api "projects/${encodedPath}" --method GET 2>/dev/null | jq -r '.id // empty'`,
			);
			if (checkResult.exitCode === 0 && checkResult.stdout.trim()) {
				projectId = checkResult.stdout.trim();
				console.warn(`GitLab project ready (id=${projectId})`);
				break;
			}
			if (attempt < 11) {
				try { execFileSync("sleep", ["5"], { cwd, timeout: 30_000 }); } catch { /* ignore */ }
			}
		}
		if (!projectId) {
			console.warn(`GitLab project not ready after 60s, continuing anyway.`);
		}
	}

	runScript(cwd, "git remote remove origin 2>/dev/null");
	const remoteUrl = getGlabRemoteUrl(repository);
	runScript(cwd, `git remote add origin "${remoteUrl}"`);
	return repository;
}

// Link a remote issue to the epic tracking issue
export function linkRemoteIssue(
	cwd: string,
	issueId: string,
	epicId: string,
): { success: boolean; error?: string } {
	const linkScript = join(cwd, ".pi/scripts/git/link-issue-to-epic.sh");
	if (!existsSync(linkScript)) {
		return { success: false, error: "link-issue-to-epic.sh not found" };
	}

	const safeIssue = issueId.replace(/[^a-zA-Z0-9 _\-.]/g, "");
	const safeEpic = epicId.replace(/[^a-zA-Z0-9 _\-.]/g, "");

	const cmd = `bash "${linkScript}" --issue-id "${safeIssue}" --epic-id "${safeEpic}"`;
	const result = runScript(cwd, cmd);
	if (result.exitCode !== 0) {
		return { success: false, error: result.stdout };
	}
	return { success: true };
}

// ── Architecture Discovery ──

export function readGroupId(cwd: string): string {
    // Try pom.xml
    const pomPath = join(cwd, "pom.xml");
    try {
        const pom = readFileSync(pomPath, "utf-8");
        const match = pom.match(/<groupId>([^<]+)<\/groupId>/);
        if (match && match[1] !== "com.example") return match[1];
    } catch {}
    // Try build.gradle
    const gradlePath = join(cwd, "build.gradle");
    try {
        const gradle = readFileSync(gradlePath, "utf-8");
        const match = gradle.match(/group\s*=\s*['"]([^'"]+)['"]/);
        if (match) return match[1];
    } catch {}
    return "com.example";
}

export function findModuleByName(cwd: string, name: string): string | null {
    const files = discoverModules(cwd);
    const nameLower = name.toLowerCase().replace(/[^a-z0-9]/g, "");

    // Pass 1: exact match
    for (const f of files) {
        const key = f.replace(".md", "").toLowerCase().replace(/[^a-z0-9]/g, "");
        if (key === nameLower) return f;
    }

    // Pass 2: prefix match ("audit" → audit-ingestion over audit-query, audit-export)
    const prefix: string[] = [];
    for (const f of files) {
        const key = f.replace(".md", "").toLowerCase().replace(/[^a-z0-9]/g, "");
        if (key.startsWith(nameLower) && key.length > nameLower.length) prefix.push(f);
    }
    if (prefix.length === 1) return prefix[0];

    // Pass 3: unambiguous substring match
    const subs: string[] = [];
    for (const f of files) {
        const key = f.replace(".md", "").toLowerCase().replace(/[^a-z0-9]/g, "");
        if (key.includes(nameLower)) subs.push(f);
    }
    if (subs.length === 1) return subs[0];

    return null;
}

export function discoverModules(cwd: string): string[] {
	const dir = join(cwd, ARCH_MODULES_DIR);
	if (!existsSync(dir)) return [];
	try {
		return readdirSync(dir).filter((f) => f.endsWith(".md"));
	} catch {
		return [];
	}
}

export function parseModuleFile(filePath: string): ModuleComponent[] {
	if (!existsSync(filePath)) return [];
	const content = readFileSync(filePath, "utf-8");
	const components: ModuleComponent[] = [];

	const lines = content.split("\n");
	let inComponentSection = false;
	let inDetailsSection = false;
	let currentName = "";
	let currentStatus = "";
	let currentDesc = "";
	let currentDeps: string[] = [];

	function saveCurrent() {
		if (currentName) {
			// Default to planned if no explicit status found
			const status = currentStatus || "planned";
			const desc = currentDesc || `${currentName} component`;
			components.push({
				name: currentName,
				status: status as ModuleComponent["status"],
				description: desc.trim(),
				dependencies: currentDeps.length > 0 ? currentDeps : ["none"],
			});
		}
	}

	for (const line of lines) {
		const trimmed = line.trim();

		// Enter component section (supports "## Components", "## Aggregates", "## Component Details")
		if (trimmed.match(/^##\s+Components?/i) || trimmed.match(/^##\s+Component\s+Details/i) || trimmed.match(/^##\s+Aggregates?/i)) {
			inComponentSection = true;
			continue;
		}

		// Leave component section on next top-level section
		if (inComponentSection && trimmed.match(/^##\s+/) && !trimmed.match(/^##\s+Components?/i)) {
			saveCurrent();
			currentName = "";
			currentStatus = "";
			currentDesc = "";
			currentDeps = [];
			inComponentSection = false;
			inDetailsSection = false;
			continue;
		}

		// Component heading (###) — start a new component entry
		if (inComponentSection && trimmed.match(/^###\s+/)) {
			// Skip non-component ### headings like "### Depends On" or "### Security"
			const name = trimmed.replace(/^###\s+/, "");
			if (name.match(/^(depends|security|testing|performance|error|change|data flow|responsibilities|overview|interfaces|inputs|outputs)/i)) {
				continue;
			}
			saveCurrent();
			currentName = name;
			currentStatus = "";
			currentDesc = "";
			currentDeps = [];
			continue;
		}

		if (!currentName) continue;

		if (trimmed.startsWith("status:")) {
			currentStatus = trimmed.replace("status:", "").trim().toLowerCase();
		} else if (trimmed.startsWith("depends:")) {
			const depsStr = trimmed.replace("depends:", "").trim();
			if (depsStr && depsStr !== "none" && depsStr !== "[TODO") {
				currentDeps = depsStr.split(",").map((d) => d.trim()).filter(Boolean);
			}
		} else if (trimmed.startsWith("**Purpose:**")) {
			currentDesc = trimmed.replace(/\*\*Purpose:\*\*\s*/, "").trim();
		} else if (!currentDesc && trimmed.length > 10 && !trimmed.startsWith("#") && !trimmed.startsWith("-") && !trimmed.startsWith("|") && !trimmed.startsWith(">") && !trimmed.startsWith("```")) {
			// Use first substantial sentence as description
			currentDesc = trimmed.slice(0, 200);
		}
	}

	// Also parse AC table rows from ## Acceptance Criteria section
	// 4-column format: | # | Component | Criterion | Verify In |
	const acMatch = content.match(/^## Acceptance Criteria[\s\S]*?(?=^## |\Z)/m);
	if (acMatch) {
		const acSection = acMatch[0];
		const acRows = acSection.match(/^\| [\d✅☐][^|]*\|[^|]+\|[^|]+\|[^|]*\|$/gm);
		if (acRows) {
			const existingNames = new Set(components.map((c) => c.name));
			for (const row of acRows) {
				const parts = row.split("|").map((p) => p.trim()).filter(Boolean);
				// 4-col: [num, component, criterion, verifyIn]
				// 3-col: [num, criterion, verifyIn]
				const isFourCol = parts.length >= 4 && parts[1] !== "Criterion";
				if (isFourCol) {
					const compName = parts[1];
					if (!existingNames.has(compName)) {
						existingNames.add(compName);
						components.push({
							name: compName,
							status: "planned",
							description: parts[2] || `${compName} component`,
							dependencies: ["none"],
						});
					}
				}
			}
		}
	}

	saveCurrent();
	return components;
}

export function findNextLogicalSlice(cwd: string, moduleFiles: string[]): ArchitectureSlice | null {
	for (const moduleFile of moduleFiles) {
		const components = parseModuleFile(join(cwd, ARCH_MODULES_DIR, moduleFile));
		const planned = components.filter((c) => c.status === "planned");
		if (planned.length > 0) {
			return {
				module: moduleFile.replace(".md", ""),
				components,
				nextLogicalSlice: planned,
			};
		}
	}
	return null;
}

// ── Roadmap Functions ──

export function parseRoadmap(cwd: string): RoadmapPhase[] {
	const path = join(cwd, ROADMAP_FILE);
	if (!existsSync(path)) return [];
	const content = readFileSync(path, "utf-8");
	const lines = content.split("\n");
	const phases: RoadmapPhase[] = [];
	let currentPhase: Partial<RoadmapPhase> | null = null;
	let inModules = false;
	let inDeps = false;
	let inMigrations = false;
	let inCriteria = false;

	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];
		const trimmed = line.trim();

		// Detect phase heading: ## Phase N: Name (Days X-Y)
		const phaseMatch = trimmed.match(/^## Phase (\d+):\s*(.+?)\s*\((Days\s*[^)]+)\)?$/i);
		if (phaseMatch) {
			if (currentPhase && currentPhase.title) {
				phases.push({
					index: currentPhase.index!,
					title: currentPhase.title!,
					goal: currentPhase.goal || "",
					days: currentPhase.days || "",
					modules: currentPhase.modules || [],
					completedModules: currentPhase.completedModules || [],
					dependencies: currentPhase.dependencies || [],
					migrations: currentPhase.migrations || [],
					criteria: currentPhase.criteria || [],
					status: "pending",
				});
			}
			currentPhase = {
				index: parseInt(phaseMatch[1], 10),
				title: phaseMatch[2].trim(),
				days: phaseMatch[3].trim(),
				modules: [],
				completedModules: [],
				dependencies: [],
				migrations: [],
				criteria: [],
			};
			inModules = false; inDeps = false; inMigrations = false; inCriteria = false;
			continue;
		}

		if (!currentPhase) continue;

		// Goal line
		const goalMatch = trimmed.match(/^\*\*Goal:\*\*\s*(.+)/i);
		if (goalMatch) {
			currentPhase.goal = goalMatch[1].trim();
			continue;
		}

		// Section headers
		if (trimmed.match(/^###\s+Modules?/i)) {
			inModules = true; inDeps = false; inMigrations = false; inCriteria = false; continue;
		}
		if (trimmed.match(/^###\s+Dependencies?/i)) {
			inModules = false; inDeps = true; inMigrations = false; inCriteria = false; continue;
		}
		if (trimmed.match(/^###\s+Database\s+Migrations?/i)) {
			inModules = false; inDeps = false; inMigrations = true; inCriteria = false; continue;
		}
		if (trimmed.match(/^###\s+Acceptance\s+Criteria/i)) {
			inModules = false; inDeps = false; inMigrations = false; inCriteria = true; continue;
		}

		// Exit section on next ### heading
		if (trimmed.startsWith("###") && !trimmed.match(/^###\s+(Modules?|Dependencies?|Database\s+Migrations?|Acceptance\s+Criteria)/i)) {
			inModules = false; inDeps = false; inMigrations = false; inCriteria = false;
		}

		// Parse module table rows: | Module | Deliverables | Doc |
		if (inModules && trimmed.startsWith("|") && !trimmed.startsWith("|---") && !trimmed.startsWith("| Module")) {
			const parts = trimmed.split("|").map((p: string) => p.trim()).filter(Boolean);
			if (parts.length >= 2) {
				currentPhase.modules!.push({
					name: parts[0],
					deliverables: parts[1] || "",
					doc: parts[2] || `.pi/architecture/modules/${parts[0].toLowerCase().replace(/[^a-z0-9]+/g, "-")}.md`,
				});
			}
		}

		// Parse dependencies: list items
		if (inDeps && trimmed.startsWith("-")) {
			currentPhase.dependencies!.push(trimmed.replace(/^[-\s]+/, "").trim());
		}

		// Parse migrations: - NNN_name: description
		if (inMigrations && trimmed.startsWith("-")) {
			currentPhase.migrations!.push(trimmed.replace(/^[-\s]+/, "").trim());
		}

		// Parse acceptance criteria: - [ ] item
		if (inCriteria && trimmed.startsWith("- [")) {
			currentPhase.criteria!.push(trimmed.replace(/^\[-\s*\]\s*/, "").trim());
		}
	}

	// Push last phase
	if (currentPhase && currentPhase.title) {
		phases.push({
			index: currentPhase.index!,
			title: currentPhase.title!,
			goal: currentPhase.goal || "",
			days: currentPhase.days || "",
			modules: currentPhase.modules || [],
			completedModules: currentPhase.completedModules || [],
			dependencies: currentPhase.dependencies || [],
			migrations: currentPhase.migrations || [],
			criteria: currentPhase.criteria || [],
			status: "pending",
		});
	}

	// Restore status from saved state
	const saved = loadRoadmapState(cwd);
	if (saved) {
		for (const phase of phases) {
			const savedPhase = saved.phases.find((p: RoadmapPhase) => p.index === phase.index);
			if (savedPhase) phase.status = savedPhase.status;
		}
	}

	return phases;
}

export function saveRoadmapState(cwd: string, state: RoadmapState): void {
	const dir = dirname(join(cwd, ROADMAP_STATE_FILE));
	if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
	writeFileSync(join(cwd, ROADMAP_STATE_FILE), JSON.stringify(state, null, 2));
}

export function loadRoadmapState(cwd: string): RoadmapState | null {
	const p = join(cwd, ROADMAP_STATE_FILE);
	if (!existsSync(p)) return null;
	try {
		return JSON.parse(readFileSync(p, "utf-8"));
	} catch {
		return null;
	}
}

export function formatRoadmapStatus(phases: RoadmapPhase[]): string {
	if (phases.length === 0) {
		return "No implementation-roadmap.md found in .pi/architecture/.";
	}
	const lines = ["## Implementation Roadmap", ""];
	for (const phase of phases) {
		const icon = phase.status === "done" ? "✅" : phase.status === "in_progress" ? "🔄" : "⏳";
		const completedModules = phase.completedModules?.length || 0;
		lines.push(`### Phase ${phase.index}: ${phase.title} ${icon}`);
		lines.push(`**Goal:** ${phase.goal}`);
		lines.push(`**Days:** ${phase.days}`);
		lines.push(`**Status:** ${phase.status}`);
		lines.push(`**Modules (${completedModules}/${phase.modules.length}):** ${phase.modules.map((m: RoadmapPhaseModule) => m.name).join(", ")}`);
		if (phase.migrations.length > 0) {
			lines.push(`**Migrations:** ${phase.migrations.length}`);
		}
		if (phase.criteria.length > 0) {
			lines.push(`**Criteria:** ${phase.criteria.length}`);
		}
		if (phase.dependencies.length > 0) {
			lines.push(`**Depends on:** ${phase.dependencies.join(", ")}`);
		}
		lines.push("");
	}

	const done = phases.filter((p) => p.status === "done").length;
	const inProgress = phases.filter((p) => p.status === "in_progress").length;
	lines.push(`**Overall:** ${done}/${phases.length} phases done, ${inProgress} in progress`);
	return lines.join("\n");
}

export function getNextPendingPhase(phases: RoadmapPhase[]): RoadmapPhase | null {
	for (const phase of phases) {
		if (phase.status !== "done") {
			// Check dependencies
			const depsMet = phase.dependencies.every((dep: string) => {
				// Dependency format: "Phase N" or "Phase N: Name"
				const depMatch = dep.match(/Phase\s+(\d+)/i);
				if (depMatch) {
					const depIdx = parseInt(depMatch[1], 10);
					const depPhase = phases.find((p: RoadmapPhase) => p.index === depIdx);
					return depPhase && depPhase.status === "done";
				}
				return true; // non-phase dependency treated as met
			});
			if (!depsMet) return null; // can't start yet
			return phase;
		}
	}
	return null; // all done
}

