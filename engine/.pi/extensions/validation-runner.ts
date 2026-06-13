/**
 * Validation Runner Extension for pi
 *
 * Runs validation scripts as pi tools and commands.
 * See coordinator.ts for guardian_scope and guardian_validate tools.
 */

type ShellResult = {
	exitCode: number;
	stdout: string;
	stderr?: string;
};

type ExtensionContext = {
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

const VALIDATORS = {
	architecture: "engine/.pi/scripts/validate-architecture.sh",
	canonical: "engine/.pi/scripts/validate-canonical.sh",
	ci: "engine/.pi/scripts/validate-ci.sh",
	integration: "engine/.pi/scripts/validate-integration.sh",
	operations: "engine/.pi/scripts/validate-operations.sh",
	security: "engine/.pi/scripts/validate-security.sh",
	tests: "engine/.pi/scripts/validate-tests.sh",
} as const;

type ValidatorName = keyof typeof VALIDATORS;

function isValidatorName(value: string): value is ValidatorName {
	return Object.hasOwn(VALIDATORS, value);
}

export default function (pi: ExtensionAPI) {
	// Session initialization
	pi.on("session_start", async (_event, ctx) => {
		ctx.ui.notify("Guardian validation runner initialized", "info");
	});

	// Register validate command — runs scripts directly via shell
	pi.registerCommand("validate", {
		description: "Run all or specific validators",
		handler: async (args, ctx) => {
			const validators =
				args.length > 0
					? args.filter(isValidatorName)
					: ["ci", "tests", "operations", "security", "canonical"];
			ctx.ui.notify(`Running validators: ${validators.join(", ")}`, "info");

			const results: Record<string, { passed: boolean; output: string }> = {};

			for (const validator of validators) {
				const scriptPath = VALIDATORS[validator];
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
