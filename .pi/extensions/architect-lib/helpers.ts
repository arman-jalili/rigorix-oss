import { execFileSync, execSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import type { ExtensionContext, EpicState, ModuleComponent, ArchitectureSlice } from "./types.ts";
// ── Constants ──

export const EPIC_STATE_KEY = ".pi/.guardian-epic-state.json";
export const ARCH_MODULES_DIR = ".pi/architecture/modules";
export const ISSUES_DIR = ".pi/issues";

// ── Helpers ──

export function log(ctx: ExtensionContext, message: string, level = "info") {
	ctx.ui.notify(message, level);
}

export function runScript(cwd: string, script: string): { exitCode: number; stdout: string } {
	try {
		const stdout = execSync(`bash -c "${script}"`, { cwd, timeout: 120_000, encoding: "utf-8" });
		return { exitCode: 0, stdout };
	} catch (e: unknown) {
		const err = e as { status?: number; stdout?: string; message?: string };
		return { exitCode: err.status ?? 1, stdout: err.stdout ?? err.message ?? "" };
	}
}

// Read repoTool from guardian-manifest.json (defaults to "gh")
export function readRepoTool(cwd: string): string {
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

// Read the repository slug (owner/repo) from guardian-manifest.json
export function readRepository(cwd: string): string | null {
	try {
		const manifestPath = join(cwd, "guardian-manifest.json");
		if (existsSync(manifestPath)) {
			const raw = readFileSync(manifestPath, "utf-8");
			const manifest = JSON.parse(raw) as {
				repository?: string;
				templateContext?: { repository?: string };
			};
			if (manifest.repository) return manifest.repository;
			if (manifest.templateContext?.repository)
				return manifest.templateContext.repository;
		}
	} catch {
		// ignore
	}
	return null;
}

/**
 * Get Git platform base URL. For GitLab, tries to detect self-hosted instances.
 */
export function getGitBaseUrl(repoTool: string): string {
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

export function commandExists(cmd: string): boolean {
	try {
		execSync(`command -v ${cmd}`, { stdio: "ignore" });
		return true;
	} catch {
		return false;
	}
}

// Try to create a remote GitHub/GitLab issue via the shell script wrapper.
// Uses execFileSync to avoid shell quoting issues with nested commands.
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

	let stdout = "";
	let exitCode = 0;
	try {
		stdout = execFileSync("bash", args, {
			cwd,
			timeout: 120_000,
			encoding: "utf-8",
		});
	} catch (e: unknown) {
		const err = e as { status?: number; stdout?: string; message?: string };
		exitCode = err.status ?? 1;
		stdout = err.stdout ?? err.message ?? "";
	}

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
		return repository;
	}

	// Remote not configured locally — ensure the remote repo exists on GitHub/GitLab
	if (repoTool === "gh") {
		runScript(
			cwd,
			`gh repo create "${repository}" --private --description "Epic: ${epicName}" 2>&1`,
		);
		// Remove stale origin if it exists but points nowhere useful
		runScript(cwd, "git remote remove origin 2>/dev/null");
		const httpsUrl = `https://github.com/${repository}.git`;
		runScript(cwd, `git remote add origin "${httpsUrl}"`);
		return repository;
	}

	// GitLab path — detect self-hosted base URL from glab config
	const glabBaseUrl = getGitBaseUrl("glab");
	runScript(
		cwd,
		`glab repo create "${repository}" --private --description "Epic: ${epicName}" 2>&1`,
	);
	runScript(cwd, "git remote remove origin 2>/dev/null");
	const httpsUrl = `${glabBaseUrl}/${repository}.git`;
	runScript(cwd, `git remote add origin "${httpsUrl}"`);
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
    for (const f of files) {
        const key = f.replace(".md", "").toLowerCase().replace(/[^a-z0-9]/g, "");
        if (key === nameLower || nameLower.includes(key) || key.includes(nameLower)) {
            return f;
        }
    }
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

		// Enter component section (supports "## Components", "## Component Details", "## Component")
		if (trimmed.match(/^##\s+Components?/i) || trimmed.match(/^##\s+Component\s+Details/i)) {
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

