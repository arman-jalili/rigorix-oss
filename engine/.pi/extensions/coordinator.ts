/**
 * Coordinator Extension for pi
 *
 * Master orchestrator for Guardian workflows.
 * Uses guardian_scope, guardian_validate, and ask_user_question tools.
 * Zero external dependencies — self-contained pi extension.
 */

type ExtensionContext = {
	ui: { notify(message: string, level?: string): void };
	shell: {
		execute(
			command: string,
			options?: { signal?: AbortSignal },
		): Promise<{ exitCode: number; stdout: string }>;
	};
	tools: { execute(name: string, params: Record<string, unknown>): Promise<unknown> };
};

type ToolCallEvent = { toolName: string; toolCallId: string; input: { command: string } };

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
};

function isToolCallEventType(name: string, event: unknown): event is ToolCallEvent {
	return (event as ToolCallEvent)?.toolName === name;
}

const Type = {
	Array: (items: unknown, options: Record<string, unknown> = {}) => ({
		...options,
		items,
		type: "array",
	}),
	Object: (properties: Record<string, unknown>) => ({ properties, type: "object" }),
	Optional: (schema: unknown) => schema,
	String: (options: Record<string, unknown> = {}) => ({ ...options, type: "string" }),
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

function classifyScope(fileCount: number, lineChanges: number): string {
	if (fileCount > 15 || lineChanges > 500) return "critical";
	if (fileCount > 5 || lineChanges > 200) return "complex";
	if (fileCount > 2 || lineChanges > 50) return "moderate";
	return "simple";
}

// ── Delegation role types ──
// Role determines whether a subagent can spawn its own subagents.
// "leaf" (default) = cannot delegate further.
// "orchestrator" = can delegate, bounded by max_spawn_depth.
const DELEGATION_ROLES = ["leaf", "orchestrator"] as const;
type DelegationRole = (typeof DELEGATION_ROLES)[number];
const DEFAULT_MAX_SPAWN_DEPTH = 1; // 1 = flat (leaf-only)

function resolveDelegationRole(role?: string): DelegationRole {
	if (role === "orchestrator") return "orchestrator";
	return "leaf";
}

function toolResult(text: string) {
	return { content: [{ type: "text" as const, text }] };
}

function toolError(text: string) {
	return { content: [{ type: "text" as const, text }], isError: true };
}

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async (_event, ctx) => {
		ctx.ui.notify("Guardian coordinator ready", "info");
	});

	// ── guardian_scope ──
	pi.registerTool({
		name: "guardian_scope",
		label: "Guardian Scope",
		description: "Classify current git diff scope using Guardian thresholds",
		parameters: Type.Object({}),
		async execute(_toolCallId, _params, signal, _onUpdate, ctx) {
			if (signal?.aborted) {
				return toolError("Scope classification aborted");
			}

			const diff = await ctx.shell.execute("git diff --numstat HEAD", { signal });
			const rows = diff.stdout.split("\n").filter((line) => line.trim());
			const fileCount = rows.length;
			const lineChanges = rows.reduce((sum, row) => {
				const [added, removed] = row.split(/\s+/);
				const a = Number.parseInt(added, 10);
				const r = Number.parseInt(removed, 10);
				return sum + (Number.isFinite(a) ? a : 0) + (Number.isFinite(r) ? r : 0);
			}, 0);

			const scope = classifyScope(fileCount, lineChanges);
			return toolResult(
				`Scope: **${scope}**\n- Files: ${fileCount}\n- Line changes: ~${lineChanges}\n\nThresholds: simple (<3 files, <50 lines) → moderate (<6 files, <200 lines) → complex (<16 files, <500 lines) → critical (16+ files or 500+ lines)`,
			);
		},
	});

	// ── guardian_validate ──
	pi.registerTool({
		name: "guardian_validate",
		label: "Guardian Validate",
		description: "Run Guardian validation scripts for specific categories",
		parameters: Type.Object({
			validators: Type.Array(Type.String(), {
				description:
					"Categories: ci, tests, operations, security, integration, architecture, canonical",
			}),
			scope: Type.Optional(
				Type.String({ description: "Scope: simple, moderate, complex, critical" }),
			),
		}),
		async execute(_toolCallId, params, signal, onUpdate, ctx) {
			const results: Record<string, { passed: boolean; output: string }> = {};
			const validators = Array.isArray(params.validators)
				? params.validators.filter((v): v is string => typeof v === "string")
				: [];

			if (validators.length === 0) {
				return toolError(
					"No validators specified. Available: ci, tests, operations, security, integration, architecture, canonical",
				);
			}

			for (const validator of validators) {
				if (signal?.aborted) break;

				if (!isValidatorName(validator)) {
					results[validator] = { passed: false, output: `Unsupported validator: ${validator}` };
					continue;
				}

				onUpdate({ content: [{ type: "text", text: `Running ${validator} validation...` }] });

				const scriptPath = VALIDATORS[validator];
				try {
					const result = await ctx.shell.execute(`bash ${scriptPath}`, { signal });
					results[validator] = { passed: result.exitCode === 0, output: result.stdout };
				} catch (error) {
					results[validator] = { passed: false, output: `Error: ${error}` };
				}
			}

			const lines: string[] = [];
			const allPassed = Object.values(results).every((r) => r.passed);

			lines.push(`## Validation Results — ${allPassed ? "✅ All Passed" : "❌ Some Failed"}\n`);
			for (const [name, result] of Object.entries(results)) {
				lines.push(`### ${name}: ${result.passed ? "✅ PASS" : "❌ FAIL"}`);
				// Show last 15 lines of output to keep it readable
				const output = result.output.trim();
				if (output) {
					const tail = output.split("\n").slice(-15).join("\n");
					lines.push(`\`\`\`\n${tail}\n\`\`\``);
				}
				lines.push("");
			}

			return toolResult(lines.join("\n"));
		},
	});

	// ── guardian_coordinate ──
	pi.registerTool({
		name: "guardian_coordinate",
		label: "Guardian Coordinate",
		description: "Orchestrate a Guardian workflow with scope classification and validation",
		parameters: Type.Object({
			task: Type.String({ description: "Task description" }),
			scope: Type.Optional(Type.String({ description: "Override scope classification" })),
			validators: Type.Optional(Type.Array(Type.String())),
		}),
		async execute(_toolCallId, params, signal, onUpdate, ctx) {
			// 1. Classify scope
			let scope = typeof params.scope === "string" ? params.scope : undefined;
			if (!scope) {
				const scopeResult = await ctx.tools.execute("guardian_scope", {});
				// Parse the text result to extract scope
				const text =
					(scopeResult as { content?: Array<{ text?: string }> })?.content?.[0]?.text ?? "";
				const match = text.match(/Scope:\s+\*\*(\w+)\*\*/);
				scope = match?.[1] ?? "moderate";
			}

			onUpdate({ content: [{ type: "text", text: `Scope: ${scope}` }] });

			// 2. Determine validators
			const validatorMap: Record<string, string[]> = {
				simple: ["ci", "canonical"],
				moderate: ["ci", "architecture", "canonical"],
				complex: ["ci", "architecture", "security", "tests", "integration", "canonical"],
				critical: [
					"ci",
					"architecture",
					"security",
					"operations",
					"tests",
					"integration",
					"canonical",
				],
			};

			const validators = Array.isArray(params.validators)
				? params.validators.filter((v): v is string => typeof v === "string")
				: (validatorMap[scope] ?? validatorMap.moderate);

			onUpdate({ content: [{ type: "text", text: `Validators: ${validators.join(", ")}` }] });

			// 3. Run validators
			const validationResults = await ctx.tools.execute("guardian_validate", { validators, scope });

			// 4. Build coordination result
			const lines: string[] = [
				"## Coordination Report",
				"",
				`**Task:** ${params.task}`,
				`**Scope:** ${scope}`,
				`**Validators:** ${validators.join(", ")}`,
				"",
				"### Next Steps",
				scope === "critical"
					? "- Request human approval before proceeding"
					: "- Proceed with implementation",
			];

			// Append validation results
			const valText =
				(validationResults as { content?: Array<{ text?: string }> })?.content?.[0]?.text ?? "";
			if (valText) {
				lines.push("", "### Validation", valText);
			}

			return toolResult(lines.join("\n"));
		},
	});

	// ── Block catastrophic commands only (lightweight safety net) ──
	// Git commit/push are allowed — agents need them for autonomous workflows.
	// Destructive operations are blocked to prevent irreversible damage.
	pi.on("tool_call", async (event) => {
		if (!isToolCallEventType("bash", event)) return;

		const cmd = event.input.command;

		// Only block truly catastrophic operations
		const catastrophic = [
			{ pattern: /(?<!\bgit\s+)\brm\s+-rf?\b/, reason: "recursive file deletion (rm -rf)" },
			{ pattern: /\bsudo\b/, reason: "elevated privileges (sudo)" },
			{ pattern: /\bgit\s+reset\s+--hard\b/, reason: "git reset --hard (discard all changes)" },
			{ pattern: /\bgit\s+clean\s+-[a-zA-Z]*f/, reason: "git clean -f (delete untracked files)" },
		];

		for (const { pattern, reason } of catastrophic) {
			if (pattern.test(cmd)) {
				return {
					block: true,
					reason: `Guardian blocked: ${reason}. Use safer alternatives or confirm with the user.`,
				};
			}
		}
	});
}
