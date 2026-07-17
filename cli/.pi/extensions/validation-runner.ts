/**
 * Canonical Reference: .pi/architecture/modules/core-libraries.md
 * Last Sync: 2026-06-02

 * Validation Runner Extension for pi
 *
 * Runs validation scripts as pi tools and commands.
 * Supports language-specific validators from scripts/languages/{lang}/.
 * See coordinator.ts for guardian_scope and guardian_validate tools.
 */

import * as fs from "node:fs";
import * as path from "node:path";

type ShellResult = {
	exitCode: number;
	stdout: string;
	stderr?: string;
};

type ExtensionContext = {
	cwd: string;
	ui: { notify(message: string, level?: string): void };
	shell: { execute(command: string, options?: { signal?: AbortSignal }): Promise<ShellResult> };
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
			handler(args: string[], ctx: ExtensionContext): unknown | Promise<unknown>;
		},
	): void;
};

const BASE_VALIDATORS: Record<string, string> = {
	architecture: ".pi/scripts/validate-architecture.sh",
	canonical: ".pi/scripts/validate-canonical.sh",
	ci: ".pi/scripts/validate-ci.sh",
	integration: ".pi/scripts/validate-integration.sh",
	operations: ".pi/scripts/validate-operations.sh",
	security: ".pi/scripts/validate-security.sh",
	tests: ".pi/scripts/validate-tests.sh",
};

type ValidatorName = string;

/**
 * Get the project language from the manifest, if available.
 */
function getProjectLanguage(cwd: string): string | null {
	try {
		const manifestPath = path.join(cwd, "guardian-manifest.json");
		if (fs.existsSync(manifestPath)) {
			const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));
			return manifest.language || null;
		}
	} catch {
		// Ignore parse errors
	}
	return null;
}

/**
 * Discover language-specific validators from scripts/languages/{lang}/ directory.
 */
function getLanguageValidators(cwd: string, language: string | null): Record<string, string> {
	if (!language) return {};

	const langDir = path.join(cwd, `.pi/scripts/languages/${language}`);
	if (!fs.existsSync(langDir)) return {};

	const validators: Record<string, string> = {};
	try {
		const files = fs.readdirSync(langDir);
		for (const file of files) {
			if (file.endsWith(".sh")) {
				const name = file.replace("validate-", "").replace(".sh", "");
				validators[`${language}:${name}`] = `.pi/scripts/languages/${language}/${file}`;
			}
		}
	} catch {
		// Ignore read errors
	}
	return validators;
}

export default function (pi: ExtensionAPI) {
	let language: string | null = null;

	// Session initialization — detect project language
	pi.on("session_start", async (_event, ctx) => {
		language = getProjectLanguage(ctx.cwd);
		if (language) {
			ctx.ui.notify(`Guardian validation runner initialized (language: ${language})`, "info");
		} else {
			ctx.ui.notify("Guardian validation runner initialized", "info");
		}
	});

	// Register validate command — runs scripts directly via shell
	pi.registerCommand("validate", {
		description: "Run all or specific validators",
		handler: async (args, ctx) => {
			// Build full validator map (base + language-specific)
			const langValidators = getLanguageValidators(ctx.cwd, language);
			const allValidators = { ...BASE_VALIDATORS, ...langValidators };

			const validators =
				args.length > 0
					? args.filter((v) => Object.hasOwn(allValidators, v))
					: Object.keys(allValidators);

			ctx.ui.notify(`Running validators: ${validators.join(", ")}`, "info");

			const results: Record<string, { passed: boolean; output: string }> = {};

			for (const validator of validators) {
				const scriptPath = allValidators[validator];
				try {
					const result = await ctx.shell.execute(`bash ${scriptPath}`);
					results[validator] = {
						passed: result.exitCode === 0,
						output: result.stdout,
					};
				} catch (error) {
					results[validator] = { passed: false, output: `Error: ${error}` };
				}
			}

			const allPassed = Object.values(results).every((r) => r.passed);
			ctx.ui.notify(
				allPassed ? "All validations passed" : "Some validations failed",
				allPassed ? "success" : "error",
			);

			return { summary: allPassed ? "All validations passed" : "Some validations failed", results };
		},
	});
}
