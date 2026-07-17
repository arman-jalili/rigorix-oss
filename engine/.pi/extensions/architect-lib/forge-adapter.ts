/**
 * Forge Adapter — Centralized forge abstraction for GitHub and GitLab.
 *
 * All forge-specific operations (auth, repo, issues, MR/PR) go through this module.
 * Use `readRepoTool()` to determine which forge is active, then call the appropriate
 * helper — null returned / empty string means "not available / not supported".
 *
 * Canonical Reference: .pi/extensions/architect-lib/helpers.ts
 * Last Sync: 2026-07-09
 */

import { execFileSync, execSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

// ── Forge Detection ──

/**
 * Read repoTool from guardian-manifest.json (defaults to "gh")
 */
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

/**
 * Read repository slug from guardian-manifest.json
 */
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
 * Read project language from guardian-manifest.json (defaults to "typescript")
 */
export function readLanguage(cwd: string): string {
	try {
		const manifestPath = join(cwd, "guardian-manifest.json");
		if (existsSync(manifestPath)) {
			const raw = readFileSync(manifestPath, "utf-8");
			const manifest = JSON.parse(raw) as { language?: string; templateContext?: { language?: string } };
			if (manifest.language) return manifest.language;
			if (manifest.templateContext?.language) return manifest.templateContext.language;
		}
	} catch {
		// fall through to default
	}
	return "typescript";
}

/**
 * URL-encode a GitLab project/group path for API calls.
 * GitLab API requires nested paths to be URL-encoded:
 *   "group/subgroup/project" -> "group%2Fsubgroup%2Fproject"
 */
export function glabEncodePath(rawPath: string): string {
	return encodeURIComponent(rawPath);
}

/**
 * Get the appropriate GitLab remote URL based on the configured protocol.
 * Supports SSH (git@) and HTTPS protocols as configured in glab CLI.
 */
export function getGlabRemoteUrl(repository: string): string {
	// Try to detect protocol from glab config
	try {
		const protocol = execSync("glab config get git_protocol 2>/dev/null", {
			encoding: "utf-8",
			timeout: 10_000,
		}).trim();
		if (protocol === "ssh") {
			// SSH format: git@gitlab.com:group/subgroup/project.git
			// Detect self-hosted instance for the host part
			let host = "gitlab.com";
			try {
				const uri = execSync("glab config get gitlab_uri 2>/dev/null", {
					encoding: "utf-8",
					timeout: 10_000,
				}).trim();
				if (uri) {
					// Extract host from URI: "https://gitlab.example.com" -> "gitlab.example.com"
					host = uri.replace(/^https?:\/\//, "").replace(/\/+$/, "");
				}
			} catch { /* use default */ }
			return `git@${host}:${repository}.git`;
		}
	} catch { /* fall through to HTTPS */ }
	// Default to HTTPS
	const baseUrl = getGitBaseUrl("glab");
	return `${baseUrl}/${repository}.git`;
}

/**
 * Get forge base URL. For GitLab, detects self-hosted instances via glab config.
 */
export function getGitBaseUrl(repoTool: string): string {
	if (repoTool === "glab") {
		try {
			const uri = execSync("glab config get gitlab_uri 2>/dev/null", {
				encoding: "utf-8",
			}).trim();
			if (uri) {
				const normalized = uri.replace(/\/+$/, "");
				// Ensure URL has a scheme, default to https://
				if (!/^https?:\/\//i.test(normalized)) {
					return `https://${normalized}`;
				}
				return normalized;
			}
		} catch {
			// fall through to default
		}
		return "https://gitlab.com";
	}
	return "https://github.com";
}

export function commandExists(cmd: string): boolean {
	try {
		// Pass cmd as bash argument ($1) to avoid shell interpolation issues
		execFileSync("bash", ["-c", `command -v "$1"`, "_", cmd], { stdio: "ignore" });
		return true;
	} catch {
		return false;
	}
}

/**
 * Run a bash script and return exit code + stdout.
 *
 * Uses execFileSync with explicit arguments to avoid shell metacharacter bugs.
 * The script is passed directly to bash -c without an outer shell layer,
 * so characters like &, $, ", and ` inside the script are handled by bash,
 * not by an intermediate sh process.
 */
export function runScript(cwd: string, script: string): { exitCode: number; stdout: string } {
	try {
		const stdout = execFileSync("bash", ["-c", script], { cwd, timeout: 120_000, encoding: "utf-8" });
		return { exitCode: 0, stdout };
	} catch (e: unknown) {
		const err = e as { status?: number; stdout?: string; message?: string };
		return { exitCode: err.status ?? 1, stdout: err.stdout ?? err.message ?? "" };
	}
}

// ── Auth Validation ──

/**
 * Verify forge CLI is authenticated AND has access to the configured repository.
 * Returns true only if both checks pass.
 */
export function verifyForgeAccess(cwd: string): { authenticated: boolean; projectAccessible: boolean } {
	const repoTool = readRepoTool(cwd);
	const repository = readRepository(cwd);
	const result = { authenticated: false, projectAccessible: false };

	if (!commandExists(repoTool)) return result;

	// Check CLI auth status
	const authResult = runScript(
		cwd,
		repoTool === "glab" ? "glab auth status 2>/dev/null" : "gh auth status 2>/dev/null",
	);
	if (authResult.exitCode !== 0) return result;
	result.authenticated = true;

	// Verify project-level access
	if (repository) {
		const encodedRepo = repoTool === "glab" ? glabEncodePath(repository) : repository;
		const projectCheck = runScript(
			cwd,
			repoTool === "glab"
				? `glab api "projects/${encodedRepo}" --method GET 2>/dev/null | jq -r '.id // empty'`
				: `gh api "repos/${repository}" --method GET 2>/dev/null | jq -r '.id // empty'`,
		);
		if (projectCheck.exitCode === 0 && projectCheck.stdout.trim().length > 0) {
			result.projectAccessible = true;
		}
	} else {
		// No repository configured — auth is sufficient
		result.projectAccessible = true;
	}

	return result;
}

// ── PR/MR Operations ──

/**
 * Check if a PR (GitHub) or MR (GitLab) exists for the given branch.
 */
export function forgePrExists(cwd: string, branch: string): boolean {
	const repoTool = readRepoTool(cwd);
	try {
		if (repoTool === "glab") {
			const result = execSync(
				`glab mr list --source-branch "${branch}" --output json 2>/dev/null`,
				{ cwd, encoding: "utf-8" },
			);
			const parsed = JSON.parse(result.trim() || "[]");
			return Array.isArray(parsed) && parsed.length > 0;
		}
		const result = execSync(
			`gh pr list --head "${branch}" --json number --jq "length" 2>/dev/null`,
			{ cwd, encoding: "utf-8" },
		);
		return parseInt(result.trim()) > 0;
	} catch {
		return false;
	}
}

/**
 * Check if a PR (GitHub) or MR (GitLab) for the given branch is merged.
 */
export function forgePrMerged(cwd: string, branch: string): boolean {
	const repoTool = readRepoTool(cwd);
	try {
		if (repoTool === "glab") {
			const result = execSync(
				`glab mr list --source-branch "${branch}" --output json 2>/dev/null`,
				{ cwd, encoding: "utf-8" },
			);
			const parsed = JSON.parse(result.trim() || "[]") as { state?: string }[];
			return Array.isArray(parsed) && parsed.some((mr) => mr.state === "merged");
		}
		const result = execSync(
			`gh pr list --head "${branch}" --json state,mergedAt --jq ".[] | select(.state==\"MERGED\") | .mergedAt" 2>/dev/null`,
			{ cwd, encoding: "utf-8" },
		);
		return result.trim().length > 0;
	} catch {
		return false;
	}
}

// ── Issue Operations ──

/**
 * Fetch issue content from remote forge or fallback to local file.
 */
export function fetchIssueContent(
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
