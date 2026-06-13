/**
 * Project Scaffolder Extension for pi
 *
 * Scaffolds greenfield project source directories, build configuration,
 * and CI pipeline FROM architecture decisions.
 *
 * This is Epic 0: the first thing run after domain exploration and architecture
 * decisions are made, before any implementation begins.
 *
 * Consistency:
 *   /domain  → discovers bounded contexts (what to build)
 *   /project → scaffolds project from architecture (how to structure it)
 *   /architect → plans and orchestrates epics (how to build it)
 *
 * Commands:
 *   /project create --lang java --buildTool maven --groupId com.mycompany
 *   /project create --lang typescript --validators ci,tests,security --dryRun
 *   /project create --lang java --buildTool gradle --force
 *   /project status
 */

import { type ExecSyncOptions, execSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import * as path from "node:path";
import { join } from "node:path";

// ── Types ──

type ExtensionContext = {
	cwd: string;
	ui: {
		notify(message: string, level?: string): void;
		setStatus(key: string, message: string | null): void;
		confirm(title: string, message: string): Promise<boolean>;
	};
};

type ExtensionAPI = {
	registerCommand(
		name: string,
		options: {
			description: string;
			handler(args: string, ctx: ExtensionContext): unknown | Promise<unknown>;
		},
	): void;
};

// ── Helpers ──

const SUPPORTED_LANGUAGES = ["typescript", "rust", "python", "go", "java"] as const;
const BUILD_TOOLS: Record<string, string[]> = {
	java: ["maven", "gradle"],
};

function detectGuardianCommand(ctx: ExtensionContext): string {
	// Try npx first, then check if local bun project
	if (existsSync(join(ctx.cwd, "node_modules", ".bin", "guardian-framework"))) {
		return "npx guardian-framework";
	}
	if (existsSync(join(ctx.cwd, "src", "index.ts"))) {
		return "bun run src/index.ts";
	}
	return "npx guardian-framework";
}

function cmd(
	base: string,
	args: Record<string, string | boolean | undefined>,
): string {
	const parts = [base];
	for (const [key, value] of Object.entries(args)) {
		if (value === undefined || value === false) continue;
		const flag = key.length === 1 ? `-${key}` : `--${key}`;
		if (value === true) {
			parts.push(flag);
		} else {
			parts.push(`${flag} "${String(value)}"`);
		}
	}
	return parts.join(" ");
}

// ── Extension ──

export default function (pi: ExtensionAPI) {
	pi.registerCommand("project", {
		description:
			"Scaffold a greenfield project from architecture decisions. " +
			"Subcommands: create, status",
		handler(args: string, ctx: ExtensionContext) {
			const trimmed = args.trim();
			const segments = trimmed.split(/\s+/);
			const subcommand = segments[0];

			// ── /project status ──
			if (subcommand === "status") {
				const archDir = join(ctx.cwd, ".pi", "architecture");
				const modulesDir = join(archDir, "modules");
				const decisionsDir = join(archDir, "decisions");
				const srcDir = join(ctx.cwd, "src");

				const lines: string[] = ["## Project Scaffolding Status", ""];

				lines.push(`Architecture directory: ${existsSync(archDir) ? "✅" : "❌"} ${archDir}`);
				const moduleCount = existsSync(modulesDir) ? readdirSync(modulesDir).filter((f: string) => f.endsWith(".md")).length : 0;
				const decisionCount = existsSync(decisionsDir) ? readdirSync(decisionsDir).filter((f: string) => f.endsWith(".md")).length : 0;
				lines.push(`  Modules: ${existsSync(modulesDir) ? `✅ ${moduleCount} module files` : "❌ not found"}`);
				lines.push(`  Decisions: ${existsSync(decisionsDir) ? `✅ ${decisionCount} ADR files` : "❌ not found"}`);
				lines.push(`Source directory: ${existsSync(srcDir) ? "✅ exists" : "⚠️  not yet scaffolded"}`);

				lines.push("", "### Available Subcommands", "");
				lines.push("  /project create --lang <language> [options]");
				lines.push("    Scaffold project from architecture decisions");
				lines.push("", "  /project status");
				lines.push("    Show current scaffolding status");

				ctx.ui.notify("Project scaffolding status", "info");
				return lines.join("\n");
			}

			// ── /project create ──
			if (subcommand === "create") {
				// Parse flags from remaining args
				const flagArgs = segments.slice(1);
				const parsed: Record<string, string | boolean> = {};
				let i = 0;
				while (i < flagArgs.length) {
					const arg = flagArgs[i];
					if (arg.startsWith("--")) {
						const key = arg.slice(2);
						if (flagArgs[i + 1] && !flagArgs[i + 1].startsWith("--")) {
							parsed[key] = flagArgs[i + 1];
							i += 2;
						} else {
							parsed[key] = true;
							i += 1;
						}
					} else if (arg.startsWith("-") && arg.length === 2) {
						const key = arg.slice(1);
						if (flagArgs[i + 1] && !flagArgs[i + 1].startsWith("-")) {
							parsed[key] = flagArgs[i + 1];
							i += 2;
						} else {
							parsed[key] = true;
							i += 1;
						}
					} else {
						i += 1;
					}
				}

				const lang = String(parsed.lang || parsed.language || "").toLowerCase();
				const buildTool = String(parsed.buildTool || "").toLowerCase() || undefined;

				// Read groupId from manifest if not passed via CLI (matches guardian CLI behavior)
				let groupId = String(parsed.groupId || parsed.group || "");
				if (!groupId) {
					try {
						const manifestPath = path.join(ctx.cwd, "guardian-manifest.json");
						if (fs.existsSync(manifestPath)) {
							const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));
							groupId = manifest.groupId || "";
						}
					} catch { /* ignore malformed manifest */ }
				}
				if (!groupId) groupId = "com.example";
				const validators = String(parsed.validators || "ci,tests");
				const dryRun = parsed.dryRun === true || parsed["dry-run"] === true || parsed.d === true;
				const force = parsed.force === true || parsed.f === true;

				// Validate language
				if (!lang || !SUPPORTED_LANGUAGES.includes(lang as typeof SUPPORTED_LANGUAGES[number])) {
					ctx.ui.notify(
						`Unsupported language: "${lang || ""}". Supported: ${SUPPORTED_LANGUAGES.join(", ")}`,
						"error",
					);
					return [
						"Usage: /project create --lang <language> [options]",
						"",
						"Required:",
						`  --lang <name>     Language: ${SUPPORTED_LANGUAGES.join(", ")}`,
						"",
						"Options:",
						"  --buildTool <name>  Build tool (maven|gradle for Java)",
						"  --groupId <name>    Package prefix (default: com.example)",
						"  --validators <list> Comma-separated (default: ci,tests)",
						"  --dryRun            Preview without writing files",
						"  --force             Override existing project guard",
					].join("\n");
				}

				// Validate build tool for language
				if (lang === "java" && buildTool && !BUILD_TOOLS.java.includes(buildTool)) {
					ctx.ui.notify(
						`Unsupported build tool: "${buildTool}". Java supports: ${BUILD_TOOLS.java.join(", ")}`,
						"error",
					);
					return "(project command handled)";
				}

				const guardianCmd = detectGuardianCommand(ctx);
				const shellArgs: Record<string, string | boolean | undefined> = {
					lang,
					buildTool,
					groupId,
					validators,
				};
				if (dryRun) shellArgs.dryRun = true;
				if (force) shellArgs.force = true;

				const fullCmd = cmd(`${guardianCmd} project create`, shellArgs);

				ctx.ui.setStatus("command", `/project create — scaffolding ${lang} project`);
				ctx.ui.notify(
					`Scaffolding ${lang} project${buildTool ? ` (${buildTool})` : ""}...`,
					"info",
				);

				try {
					const result = execSync(fullCmd, {
						cwd: ctx.cwd,
						encoding: "utf-8",
						timeout: 60000,
						stdio: ["pipe", "pipe", "pipe"],
					} as ExecSyncOptions);

					ctx.ui.setStatus("command", null);
					ctx.ui.notify(
						dryRun ? "Dry run complete" : "Project scaffolding complete",
						"success",
					);

					return [
						"## Project Scaffolding Result",
						"",
						`Language: ${lang}`,
						buildTool ? `Build Tool: ${buildTool}` : null,
						`Group ID: ${groupId}`,
						`Validators: ${validators}`,
						dryRun ? "Mode: **Dry Run** (no files written)" : "Mode: **Live**",
						"",
						"```",
						result.trim(),
						"```",
						"",
						dryRun
							? "Run without --dryRun to create the project."
							: "Project scaffolded. Next: create architecture module docs and use /architect to plan epics.",
					]
						.filter(Boolean)
						.join("\n");
				} catch (err) {
					ctx.ui.setStatus("command", null);
					const errorMsg = err instanceof Error ? err.message : String(err);
					ctx.ui.notify(`Project scaffolding failed: ${errorMsg}`, "error");
					return `Project scaffolding failed:\n\`\`\`\n${errorMsg}\n\`\`\``;
				}
			}

			// ── Unknown subcommand ──
			ctx.ui.notify(
				[
					"Usage:",
					"  /project create --lang java --buildTool maven --groupId com.mycompany",
					"  /project create --lang typescript --validators ci,tests,security --dryRun",
					"  /project status",
				].join("\n"),
				"info",
			);

			return [
				"Available /project subcommands:",
				"",
				"  /project create --lang <language> [options]",
				"    Scaffold project from architecture decisions",
				"    Required: --lang (typescript, rust, python, go, java)",
				"    Options: --buildTool, --groupId, --validators, --dryRun, --force",
				"",
				"  /project status",
				"    Show current scaffolding status",
			].join("\n");
		},
	});
}
