/**
 * Canonical Reference: .pi/architecture/modules/core-libraries.md
 * Implements: Domain Explore pi extension — /domain command + domain_explore/domain_validate tools
 * Last Architecture Sync: 2026-05-31
 *
 * Pi extension providing:
 *   /domain --explore — Returns DDD analysis instructions + agent writes files directly
 *   /domain --architect-scaffold — Generate architecture directories from exploration
 *   /domain --validate — Validate exploration session structure
 *   domain_explore tool — (deprecated, use /domain --explore instead)
 *   domain_validate tool — Validate exploration sessions against glossary + source code
 *   domain_save_result tool — (fallback) Save agent's domain JSON as structured session
 *
 * No LLM SDKs. No API keys. The prompt IS the interface — the agent reads it and acts.
 */

import * as child_process from "node:child_process";
import * as crypto from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import { Type } from "typebox";

// ── Minimal pi ExtensionAPI types (same pattern as coordinator.ts) ──

type ShellResult = {
	exitCode: number;
	stdout: string;
};

type ExtensionContext = {
	cwd: string;
	shell: {
		execute(command: string, options?: { signal?: AbortSignal }): Promise<ShellResult>;
	};
	ui: {
		notify(message: string, level?: string): void;
	};
	tools: {
		execute(name: string, params: Record<string, unknown>): Promise<unknown>;
	};
};

/**
 * Execute a shell command, preferring ctx.shell if available,
 * falling back to child_process.execSync otherwise.
 */
function shellExec(
	ctx: ExtensionContext,
	command: string,
): { exitCode: number; stdout: string; stderr?: string } {
	if (ctx.shell?.execute) {
		return ctx.shell.execute(command) as unknown as { exitCode: number; stdout: string; stderr?: string };
	}
	try {
		const stdout = child_process.execSync(command, { encoding: "utf-8", stdio: ["pipe", "pipe", "pipe"] });
		return { exitCode: 0, stdout };
	} catch (err: unknown) {
		const execErr = err as { stdout?: string; stderr?: string; status?: number };
		return {
			exitCode: execErr.status ?? 1,
			stdout: execErr.stdout?.toString() ?? "",
			stderr: execErr.stderr?.toString() ?? "",
		};
	}
}

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
	registerCommand(name: string, options: {
		description: string;
		handler(args: string[], ctx: ExtensionContext): unknown | Promise<unknown>;
	}): void;
	sendMessage<T = unknown>(
		message: { customType?: string; content: string; display?: boolean; details?: Record<string, unknown> },
		options?: { deliverAs?: "steer" | "followUp" | "nextTurn"; triggerTurn?: boolean },
	): void;
	sendUserMessage(
		content: string,
		options?: { deliverAs?: "steer" | "followUp" },
	): void;
};

// ── Helpers ──

function toolResult(text: string) {
	return { content: [{ type: "text" as const, text }] };
}

function sanitizeContext(context: string): string {
	return context
		.replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, "")
		.replace(/\s+/g, " ")
		.trim()
		.slice(0, 5000);
}

function buildExplorationPrompt(context: string): string {
	return [
		"You are a Domain-Driven Design expert. Analyze the following business",
		"description and extract a structured domain model. Respond with valid JSON only.",
		"",
		"## Business Context",
		"",
		context,
		"",
		"## Output Format (JSON)",
		"",
		"```json",
		"{",
		'  "businessContext": "Brief one-line summary",',
		'  "actors": [',
		"    {",
		'      "name": "ActorName",',
		'      "description": "Who this actor is",',
		'      "interactions": "What they do in the system"',
		"    }",
		"  ],",
		'  "functionalRequirements": [',
		"    {",
		'      "id": "FR-001",',
		'      "requirement": "The system shall...",',
		'      "priority": "critical|high|medium|low",',
		'      "boundedContext": "ContextName"',
		"    }",
		"  ],",
		'  "nonFunctionalRequirements": [',
		"    {",
		'      "id": "NFR-001",',
		'      "requirement": "The system shall...",',
		'      "category": "performance|security|scalability|availability|maintainability",',
		'      "target": "Specific measurable target"',
		"    }",
		"  ],",
		'  "assumptions": [',
		"    {",
		'      "assumption": "We assume that...",',
		'      "impactIfWrong": "What breaks if this is false",',
		'      "mitigation": "How we handle it being wrong"',
		"    }",
		"  ],",
		'  "boundedContexts": [',
		"    {",
		'      "name": "ContextName",',
		'      "description": "What this context does",',
		'      "entities": ["EntityName1", "EntityName2"]',
		"    }",
		"  ],",
		'  "entities": [',
		"    {",
		'      "name": "EntityName",',
		'      "context": "BoundedContextName",',
		'      "type": "entity | value-object | aggregate-root",',
		'      "description": "What this entity represents"',
		"    }",
		"  ],",
		'  "domainEvents": [',
		"    {",
		'      "name": "EventName",',
		'      "context": "BoundedContextName",',
		'      "description": "What happened",',
		'      "triggeredBy": "What caused this event"',
		"    }",
		"  ],",
		'  "ubiquitousLanguage": [',
		"    {",
		'      "term": "TermName",',
		'      "definition": "Clear definition",',
		'      "boundedContext": "BoundedContextName",',
		'      "aliases": ["bad-alias-1", "bad-alias-2"],',
		'      "examples": "code snippet showing correct usage"',
		"    }",
		"  ],",
		'  "openQuestions": "Any ambiguities that need human clarification",',
		'  "aggregateRoots": ["AggregateRootName1", "AggregateRootName2"]',
		"}",
		"```",
	].join("\n");
}

// ── Domain Explore Tool (deprecated) ──

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async (_event, ctx) => {
		ctx.ui.notify("Domain explorer ready — use /domain --explore", "info");
	});

	// domain_explore (DEPRECATED — use /domain --explore command instead)
	pi.registerTool({
		name: "domain_explore",
		label: "Domain Explore",
		description:
			"[DEPRECATED] Use /domain --explore instead. " +
			"Creates a DDD domain exploration prompt file.",
		parameters: Type.Object({
			context: Type.String({ description: "Business domain description to explore" }),
			sessionId: Type.Optional(Type.String({ description: "Optional custom session ID" })),
			dryRun: Type.Optional(Type.Boolean({ description: "Simulate without writing files" })),
		}),
		async execute(_toolCallId, params, _signal, onUpdate, ctx) {
			const context = String(params.context ?? "").trim();
			const sessionId = String(params.sessionId ?? "") || crypto.randomUUID();
			const dryRun = params.dryRun === true;

			if (!context) {
				return toolResult("ERROR: context is required (business description)");
			}

			const sanitized = sanitizeContext(context);
			const prompt = buildExplorationPrompt(sanitized);
			const explorationDir = path.join(ctx.cwd, ".pi", "domain", "exploration");
			const promptPath = path.join(explorationDir, sessionId + ".prompt.md");

			if (!dryRun) {
				fs.mkdirSync(explorationDir, { recursive: true });
				fs.writeFileSync(promptPath, prompt, "utf-8");
			}

			let existingSessions = 0;
			try {
				if (fs.existsSync(explorationDir)) {
					existingSessions = fs.readdirSync(explorationDir).filter(
						(f) => f.endsWith(".md") && !f.includes(".prompt."),
					).length;
				}
			} catch {
				existingSessions = 0;
			}

			const result = [
				"[DEPRECATED — use /domain --explore instead]",
				"Domain Exploration Created",
				"Session ID: " + sessionId,
				"Prompt File: " + (dryRun ? "[dry-run, not written]" : promptPath),
				"Context Length: " + sanitized.length + " characters",
				"Status: awaiting-response",
				"Existing Sessions: " + existingSessions,
			];

			return toolResult(result.join("\n"));
		},
	});

	// domain_save_result (fallback — writes structured files from agent JSON)
	pi.registerTool({
		name: "domain_save_result",
		label: "Domain Save Result",
		description:
			"Save the agent's domain analysis JSON as a structured exploration session. " +
			"As a fallback for agents that prefer tool calls over direct file writes. " +
			"Parses JSON, writes exploration.md and updates ubiquitous-language.md.",
		parameters: Type.Object({
			sessionId: Type.String({ description: "Session ID from --explore" }),
			responseJson: Type.String({ description: "Domain analysis JSON from the agent" }),
		}),
		async execute(_toolCallId, params, _signal, onUpdate, ctx) {
			const sessionId = String(params.sessionId ?? "").trim();
			const responseJson = String(params.responseJson ?? "").trim();
			if (!sessionId || !responseJson) {
				return { content: [{ type: "text" as const, text: "ERROR: sessionId and responseJson are required" }] };
			}
			let cleaned = responseJson.trim();
			const jsonMatch = cleaned.match(/```(?:json)?\s*([\s\S]*?)```/);
			if (jsonMatch) cleaned = jsonMatch[1].trim();
			let parsedJson: Record<string, unknown>;
			try { parsedJson = JSON.parse(cleaned) as Record<string, unknown>; }
			catch (e) { return { content: [{ type: "text" as const, text: "ERROR: Failed to parse JSON: " + String(e) }] }; }

			const now = new Date().toISOString().split("T")[0];
			const explorationDir = path.join(ctx.cwd, ".pi", "domain", "exploration");
			fs.mkdirSync(explorationDir, { recursive: true });

			function toRow(arr: unknown[], fields: string[]): string {
				if (!Array.isArray(arr) || arr.length === 0) return "None identified yet.";
				return arr.map((item: Record<string, unknown>) => {
					const vals = fields.map((f) => String(item[f] ?? "").replace(/\n/g, " "));
					return "| " + vals.join(" | ") + " |";
				}).join("\n");
			}

			const bc = String(parsedJson.businessContext ?? "").replace(/"/g, '\\"');
			const actors = toRow(parsedJson.actors as unknown[], ["name", "description", "interactions"]);
			const fr = toRow(parsedJson.functionalRequirements as unknown[], ["id", "requirement", "priority", "boundedContext"]);
			const nfr = toRow(parsedJson.nonFunctionalRequirements as unknown[], ["id", "requirement", "category", "target"]);
			const asmp = toRow(parsedJson.assumptions as unknown[], ["assumption", "impactIfWrong", "mitigation"]);
			const bcs = toRow(parsedJson.boundedContexts as unknown[], ["name", "description", "entities"]);
			const ents = toRow(parsedJson.entities as unknown[], ["name", "context", "type", "description"]);
			const evts = toRow(parsedJson.domainEvents as unknown[], ["name", "context", "description", "triggeredBy"]);
			const ul = toRow(parsedJson.ubiquitousLanguage as unknown[], ["term", "definition", "boundedContext", "aliases"]);
			const oq = String(parsedJson.openQuestions ?? "None");
			const ar = String(parsedJson.aggregateRoots ?? "None");

			const sessionContent = [
				"---", "session_id: " + sessionId, "created: " + now,
				'business_context: "' + bc + '"', "status: draft", "---", "",
				"# Domain Exploration: " + sessionId, "",
				"> **Status:** draft — AI-suggested, human-review needed.", "",
				"---", "", "## Business Context", "", bc,
				"", "---", "", "## Actors & Roles",
				"", "| Actor | Description | Interactions |", "|-------|-------------|-------------|", actors,
				"", "---", "", "## Functional Requirements",
				"", "| ID | Requirement | Priority | Bounded Context |", "|----|-------------|----------|----------------|", fr,
				"", "---", "", "## Non-Functional Requirements",
				"", "| ID | Requirement | Category | Target |", "|----|-------------|----------|--------|", nfr,
				"", "---", "", "## Assumptions",
				"", "| Assumption | Impact if Wrong | Mitigation |", "|------------|----------------|-----------|", asmp,
				"", "---", "", "## Bounded Contexts",
				"", "| Context | Description | Entities |", "|---------|-------------|----------|", bcs,
				"", "---", "", "## Entities",
				"", "| Entity | Context | Type | Description |", "|--------|---------|------|-------------|", ents,
				"", "---", "", "## Domain Events",
				"", "| Event | Context | Description | Triggered By |", "|-------|---------|-------------|-------------|", evts,
				"", "---", "", "## Ubiquitous Language",
				"", "| Term | Definition | Bounded Context | Aliases/Synonyms |", "|------|-----------|----------------|-----------------|", ul,
				"", "---", "", "## Open Questions", "", oq,
				"", "---", "", "## Aggregate Roots", "", ar,
			].join("\n");

			// Write exploration.md (the canonical rendered output)
			const explorationMdPath = path.join(ctx.cwd, ".pi", "domain", "exploration.md");
			const tmpExploration = explorationMdPath + ".tmp";
			fs.writeFileSync(tmpExploration, sessionContent, "utf-8");
			fs.renameSync(tmpExploration, explorationMdPath);

			// Also write session file
			const sessionPath = path.join(explorationDir, sessionId + ".md");
			const tmpSession = sessionPath + ".tmp";
			fs.writeFileSync(tmpSession, sessionContent, "utf-8");
			fs.renameSync(tmpSession, sessionPath);

			// Update ubiquitous-language.md with new terms
			const glPath = path.join(ctx.cwd, ".pi", "domain", "ubiquitous-language.md");
			const ulRaw = parsedJson.ubiquitousLanguage as Array<Record<string, unknown>> | undefined;
			if (Array.isArray(ulRaw) && ulRaw.length > 0) {
				let glContent = "";
				if (fs.existsSync(glPath)) glContent = fs.readFileSync(glPath, "utf-8");
				else glContent = "# Ubiquitous Language\n\n## Glossary\n\n| Term | Definition | Bounded Context | Aliases/Synonyms | Examples |\n|------|-----------|----------------|-----------------|---------|";
				const existing = new Set<string>();
				for (const line of glContent.split("\n")) {
					if (line.startsWith("| ") && !line.includes("| Term |") && !line.includes("|---|")) {
						const t = line.split("|")[1]?.trim()?.toLowerCase();
						if (t) existing.add(t);
					}
				}
				const rows: string[] = [];
				for (const t of ulRaw) {
					const tn = String(t.term ?? "").trim();
					if (!tn || existing.has(tn.toLowerCase())) continue;
					rows.push("| " + tn + " | " + String(t.definition ?? "").replace(/\n/g, " ") + " | " + String(t.boundedContext ?? "") + " | " + (Array.isArray(t.aliases) ? t.aliases.join(", ") : "") + " | " + String(t.examples ?? "") + " |");
					existing.add(tn.toLowerCase());
				}
				if (rows.length > 0) {
					const lines = glContent.split("\n");
					let last = -1;
					for (let i = lines.length - 1; i >= 0; i--) {
						if (lines[i].startsWith("| ") && !lines[i].includes("|---|") && !lines[i].includes("| Term |")) { last = i; break; }
					}
					if (last >= 0) lines.splice(last + 1, 0, ...rows);
					else { for (let i = 0; i < lines.length; i++) { if (lines[i].includes("|---|")) { lines.splice(i + 1, 0, ...rows); break; } } }
					const gt = glPath + ".tmp";
					fs.writeFileSync(gt, lines.join("\n"), "utf-8");
					fs.renameSync(gt, glPath);
				}
			}

			onUpdate({ content: [{ type: "text", text: "Domain exploration saved for session: " + sessionId }] });
			return { content: [{ type: "text" as const, text: "Domain exploration saved for " + sessionId + ". Next: /domain --architect-scaffold " + sessionId + " or review .pi/domain/exploration.md" }] };
		},
	});

	// domain_validate
	pi.registerTool({
		name: "domain_validate",
		label: "Domain Validate",
		description:
			"Validate a domain exploration session against the canonical glossary and " +
			"source code. Checks: file exists, structural integrity, glossary compliance, " +
			"source drift, and canonical reference integrity.",
		parameters: Type.Object({
			sessionId: Type.String({ description: "Exploration session ID to validate" }),
		}),
		async execute(_toolCallId, params, _signal, onUpdate, ctx) {
			const sessionId = String(params.sessionId ?? "").trim();
			if (!sessionId) return toolResult("ERROR: sessionId is required");

			const explorationDir = path.join(ctx.cwd, ".pi", "domain", "exploration");
			const sessionPath = path.join(explorationDir, sessionId + ".md");
			const glossaryPath = path.join(ctx.cwd, ".pi", "domain", "ubiquitous-language.md");
			const checks: string[] = [];
			let allPassed = true;

			function addCheck(name: string, passed: boolean, detail?: string) {
				const icon = passed ? "PASS" : "FAIL";
				if (!passed) allPassed = false;
				checks.push("  " + icon + " " + name + (detail ? " - " + detail : ""));
			}

			// 1. Session file exists
			if (!fs.existsSync(sessionPath)) {
				addCheck("Session file", false, "Not found");
			} else {
				addCheck("Session file", true, path.basename(sessionPath));
			}

			// 2. Parse and validate structure
			if (fs.existsSync(sessionPath)) {
				const content = fs.readFileSync(sessionPath, "utf-8");
				const hasBoundedContexts = content.includes("## Bounded Contexts");
				const hasEntities = content.includes("## Entities");
				const hasGlossary = content.includes("## Ubiquitous Language");
				addCheck("Bounded contexts section", hasBoundedContexts);
				addCheck("Entities section", hasEntities);
				addCheck("Ubiquitous language section", hasGlossary);
				addCheck("Structural integrity", hasBoundedContexts && hasEntities && hasGlossary);
			}

			// 3. Glossary compliance
			if (fs.existsSync(glossaryPath)) {
				const content = fs.readFileSync(glossaryPath, "utf-8");
				const terms = content.split("\n").filter(l => l.startsWith("|") && !l.includes("|---") && !l.includes("| Term |")).length;
				addCheck("Glossary parsed", terms > 0, terms + " canonical terms");
			} else {
				addCheck("Glossary file", false, "Not found");
			}

			// 4. Source code drift
			const scriptPath = path.join(ctx.cwd, ".pi", "scripts", "validate-ubiquitous-language.sh");
			if (fs.existsSync(scriptPath)) {
				try {
					const result = await shellExec(ctx, "bash " + scriptPath);
					addCheck("Source drift", result.exitCode === 0, result.stdout.slice(-80).trim());
				} catch (err) {
					addCheck("Source drift", false, "Error: " + String(err));
				}
			} else {
				addCheck("Source drift check skipped", true, "script not found");
			}

			const header = allPassed
				? "Domain Validation - All Checks Passed"
				: "Domain Validation - Some Checks Failed";
			return toolResult(header + "\n" + checks.join("\n"));
		},
	});

	// ── /domain command ──
	pi.registerCommand("domain", {
		description:
			"Domain exploration commands. Subcommands: --explore, --architect-scaffold, --validate",
		async handler(args: string[], ctx: ExtensionContext) {
			const trimmed = Array.isArray(args) ? args.join(" ").trim() : String(args).trim();

			// /domain --explore "context description"
			if (trimmed.startsWith("--explore")) {
				const context = trimmed.slice("--explore".length).trim().replace(/^["']|["']$/g, "");
				if (!context) {
					ctx.ui.notify(
						'Usage: /domain --explore "Business domain description"',
						"error",
					);
					return "(domain command handled)";
				}
				const sanitized = sanitizeContext(context);
				const prompt = buildExplorationPrompt(sanitized);
				const sessionId = crypto.randomUUID();
				const explorationDir = path.join(ctx.cwd, ".pi", "domain", "exploration");
				fs.mkdirSync(explorationDir, { recursive: true });

				// Create session file with business context
				const sessionPath = path.join(explorationDir, sessionId + ".md");
				const initialContent = [
					"---",
					"session_id: " + sessionId,
					"created: " + new Date().toISOString().split("T")[0],
					'business_context: "' + sanitized.replace(/"/g, '\\"') + '"',
					"status: draft",
					"---",
					"",
					"# Domain Exploration: " + sessionId,
					"",
					"> **Status:** agent analysis requested",
					"",
					"---",
					"",
					"## Business Context",
					"",
					sanitized,
				].join("\n");
				fs.writeFileSync(sessionPath, initialContent, "utf-8");

				// Write stub exploration.md with business context filled in
				const explorationMdPath = path.join(ctx.cwd, ".pi", "domain", "exploration.md");
				const stubContent = [
					"---",
					"session_id: " + sessionId,
					"created: " + new Date().toISOString().split("T")[0],
					'business_context: "' + sanitized.replace(/"/g, '\\"') + '"',
					"status: draft",
					"---",
					"",
					"# Domain Exploration: " + sessionId,
					"",
					"> **Status:** draft — agent needs to fill in the analysis below.",
					"",
					"---",
					"",
					"## Business Context",
					"",
					sanitized,
					"",
					"---",
					"",
					"## Actors & Roles",
					"",
					"| Actor | Description | Interactions |",
					"|-------|-------------|-------------|",
					"| | | |",
					"",
					"---",
					"",
					"## Functional Requirements",
					"",
					"| ID | Requirement | Priority | Bounded Context |",
					"|----|-------------|----------|----------------|",
					"| | | | |",
					"",
					"---",
					"",
					"## Non-Functional Requirements",
					"",
					"| ID | Requirement | Category | Target |",
					"|----|-------------|----------|--------|",
					"| | | | |",
					"",
					"---",
					"",
					"## Assumptions",
					"",
					"| Assumption | Impact if Wrong | Mitigation |",
					"|------------|----------------|-----------|",
					"| | | |",
					"",
					"---",
					"",
					"## Bounded Contexts",
					"",
					"| Context | Description | Entities |",
					"|---------|-------------|----------|",
					"| | | |",
					"",
					"---",
					"",
					"## Entities",
					"",
					"| Entity | Context | Type | Description |",
					"|--------|---------|------|-------------|",
					"| | | | |",
					"",
					"---",
					"",
					"## Domain Events",
					"",
					"| Event | Context | Description | Triggered By |",
					"|-------|---------|-------------|-------------|",
					"| | | | |",
					"",
					"---",
					"",
					"## Ubiquitous Language",
					"",
					"| Term | Definition | Bounded Context | Aliases/Synonyms |",
					"|------|-----------|----------------|-----------------|",
					"| | | | |",
					"",
					"---",
					"",
					"## Open Questions",
					"",
					"",
					"---",
					"",
					"## Aggregate Roots",
					"",
					"",
				].join("\n");
				fs.writeFileSync(explorationMdPath + ".tmp", stubContent, "utf-8");
				fs.renameSync(explorationMdPath + ".tmp", explorationMdPath);

				// Create ubiquitous-language.md if it doesn't exist
				const glPath = path.join(ctx.cwd, ".pi", "domain", "ubiquitous-language.md");
				if (!fs.existsSync(glPath)) {
					const glContent = [
						"# Ubiquitous Language",
						"",
						"> Canonical glossary for this project.",
						"> All code MUST use these terms. Aliases/synonyms listed below are **prohibited** in source identifiers.",
						"",
						"## Glossary",
						"",
						"| Term | Definition | Bounded Context | Aliases/Synonyms | Examples |",
						"|------|-----------|----------------|-----------------|---------|",
						"| | | | | |",
					].join("\n");
					fs.writeFileSync(glPath + ".tmp", glContent, "utf-8");
					fs.renameSync(glPath + ".tmp", glPath);
				}

				ctx.ui.notify(
					"Domain analysis requested: " + sessionId,
					"success",
				);

				// Inject the DDD analysis prompt as a follow-up message to the agent.
				// Using sendMessage with triggerTurn=true causes the agent to process
				// this as a new conversation turn, not as a command response.
				const analysisPrompt = [
					"I need you to analyze the following business domain using Domain-Driven Design.",
					"I have created stub files in .pi/domain/exploration.md and .pi/domain/ubiquitous-language.md.",
					"",
					"Your task:",
					"1. READ .pi/domain/exploration.md — it has the business context and empty tables",
					"2. ANALYZE the domain: actors, FR/NFR, assumptions, bounded contexts, entities, events , glossary",
					"3. FILL IN all empty tables in .pi/domain/exploration.md with your analysis",
					"4. UPDATE .pi/domain/ubiquitous-language.md with the glossary terms",
					"",
					"Business Context:",
					sanitized,
					"",
					"Use this JSON schema as a reference for structuring your analysis:",
					'```json',
					"{",
					'  "actors": [{ "name": "", "description": "", "interactions": "" }],',
					'  "functionalRequirements": [{ "id": "FR-001", "requirement": "", "priority": "critical|high|medium|low", "boundedContext": "" }],',
					'  "nonFunctionalRequirements": [{ "id": "NFR-001", "requirement": "", "category": "performance|security|scalability|availability|maintainability", "target": "" }],',
					'  "assumptions": [{ "assumption": "", "impactIfWrong": "", "mitigation": "" }],',
					'  "boundedContexts": [{ "name": "", "description": "", "entities": [""] }],',
					'  "entities": [{ "name": "", "context": "", "type": "entity|value-object|aggregate-root", "description": "" }],',
					'  "domainEvents": [{ "name": "", "context": "", "description": "", "triggeredBy": "" }],',
					'  "ubiquitousLanguage": [{ "term": "", "definition": "", "boundedContext": "", "aliases": [""], "examples": "" }]',
					"}",
					'```',
				].join("\n");

				try {
					pi.sendMessage(
						{ content: analysisPrompt, display: true },
						{ deliverAs: "followUp", triggerTurn: true },
					);
				} catch (e) {
					// Fallback: if sendMessage is unavailable, return the prompt directly
					return analysisPrompt;
				}

				return "Domain analysis dispatched to agent. Session: " + sessionId;
			}

			// /domain --architect-scaffold <session-id>
			// /domain --architect-scaffold <session-id>
			if (trimmed.startsWith("--architect-scaffold")) {
				const sessionId = trimmed.slice("--architect-scaffold".length).trim();
				if (!sessionId) {
					ctx.ui.notify(
						"Usage: /domain --architect-scaffold <session-id>",
						"error",
					);
					return "(domain command handled)";
				}

				const explorationDir = path.join(ctx.cwd, ".pi", "domain", "exploration");
				const sessionPath = path.join(explorationDir, sessionId + ".md");

				if (!fs.existsSync(sessionPath)) {
					ctx.ui.notify(
						"Session not found: " + sessionId + ". Run /domain --explore first.",
						"error",
					);
					return "(domain command handled)";
				}

				// Create architecture directories
				const archDir = path.join(ctx.cwd, ".pi", "architecture");
				const modulesDir = path.join(archDir, "modules");
				const decisionsDir = path.join(archDir, "decisions");
				const diagramsDir = path.join(archDir, "diagrams");

				fs.mkdirSync(modulesDir, { recursive: true });
				fs.mkdirSync(decisionsDir, { recursive: true });
				fs.mkdirSync(diagramsDir, { recursive: true });

				// Read the rendered exploration.md (agent-filled) and the session file
				const sessionContent = fs.readFileSync(sessionPath, "utf-8");
				const explorationMdPath = path.join(ctx.cwd, ".pi", "domain", "exploration.md");
				const timestamp = new Date().toISOString().split("T")[0];

				// If exploration.md exists and has analysis sections, sync them into the session file
				let analysisContent = "";
				if (fs.existsSync(explorationMdPath)) {
					analysisContent = fs.readFileSync(explorationMdPath, "utf-8");
					// Only update session file if exploration.md has analysis beyond business context
					if (analysisContent.includes("## Actors") || analysisContent.includes("## Bounded Contexts")) {
						// Preserve the session file's frontmatter, replace the body with exploration content
						const frontMatch = sessionContent.match(/^---\n[\s\S]*?\n---/);
						const analysisBody = analysisContent.match(/^---\n[\s\S]*?\n---\n([\s\S]*)$/);
						if (frontMatch && analysisBody) {
							const updatedSession = frontMatch[0] + "\n" + analysisBody[1];
							const tmp = sessionPath + ".tmp";
							fs.writeFileSync(tmp, updatedSession, "utf-8");
							fs.renameSync(tmp, sessionPath);
						}
					}
				}

				// Parse bounded context names from (now synced) session content
				const bcNames: string[] = [];
				const sourceForBC = analysisContent || sessionContent;
				const bcSection = sourceForBC.match(/## Bounded Contexts[\s\S]*?(?=\n## |$)/);
				if (bcSection) {
					const bcLines = bcSection[0].split("\n");
					let inData = false;
					for (const line of bcLines) {
						if (line.includes("|---")) { inData = true; continue; }
						if (!inData || !line.startsWith("|")) continue;
						const cells = line.split("|").map(c => c.trim()).filter(c => c);
						if (cells.length >= 1 && cells[0] !== "Context" && cells[0] !== "Bounded Context") {
							bcNames.push(cells[0]);
						}
					}
				}

				// Step 1: Call guardian CLI to scaffold module docs per bounded context
				let scaffoldModules: string[] = [];
				let scaffoldWarnings: string[] = [];
				let scaffoldError = "";

				try {
					const cmd = "guardian-framework domain scaffold " + sessionId + " 2>&1";
					const shellResult = await shellExec(ctx, cmd);
					if (shellResult.exitCode === 0) {
						const outputLines = shellResult.stdout.split("\n");
						for (const line of outputLines) {
							const t = line.trim();
							if (t.endsWith(".md") && !t.startsWith("Warnings:")) {
								scaffoldModules.push(t);
							}
							if (t.startsWith("Warnings:") || t.includes("already exists")) {
								scaffoldWarnings.push(t);
							}
						}
						ctx.ui.notify("Generated " + scaffoldModules.length + " module doc(s) from exploration", "success");
					} else {
						scaffoldError = shellResult.stdout.slice(0, 200);
						ctx.ui.notify("Module scaffold warning: " + scaffoldError.slice(0, 80), "info");
					}
				} catch (e) {
					scaffoldError = String(e).slice(0, 200);
					ctx.ui.notify("Module scaffold unavailable - agent can create docs directly", "info");
				}

				// Step 2: Always regenerate ADR-001 to reflect current session
				const adrPath = path.join(decisionsDir, "ADR-001-architecture-pattern.md");
				const bcList = bcNames.length > 0
				? bcNames.map(n => "  - " + n).join("\n")
				: "  - (to be defined during architecture planning)";

				const adrContent = [
				"# ADR-001: Domain-Driven Design with Bounded Contexts",
				"",
				"**Status:** Proposed",
				"**Date:** " + timestamp,
				"**Session:** " + sessionId,
				"",
				"## Context",
				"",
				"The domain exploration identified bounded contexts that must be",
				"implemented as independently evolvable modules.",
				"",
				bcList,
				"",
				"## Decision",
				"",
				"We will use Domain-Driven Design with bounded contexts as independently",
				"evolvable modules (Modular Monolith pattern).",
				"",
				"## Consequences",
				"",
				"- Each bounded context owns its data and domain logic",
				"- Cross-context communication through domain events",
				"- Contexts can be extracted to separate services when needed",
				"- Disciplined dependency management required",
				"",
				"## Alternatives Considered",
				"",
				"- Monolith without domain boundaries: rejected - no separation of concerns",
				"- Microservices from day one: rejected - over-engineering for initial scope",
				"- Layered Architecture: rejected - does not enforce domain boundaries",
				"",
				"## Affected Modules",
				"",
				bcList,
				].join("\n");

				const adrTmp = adrPath + ".tmp";
				fs.writeFileSync(adrTmp, adrContent, "utf-8");
				fs.renameSync(adrTmp, adrPath);

				// Step 3: Generate system context diagram
				const diagramPath = path.join(diagramsDir, "system-context.md");
				let contextName = "System";
				const ctxMatch = sessionContent.match(/business_context:\s*"(.+?)"/);
				if (ctxMatch) {
					contextName = ctxMatch[1].split(".")[0].slice(0, 80);
				}

				// Build mermaid diagram from bounded contexts
				// Labels are double-quoted to handle special chars like (), [], {}
				let bcDiagram = "";
				if (bcNames.length > 0) {
					const escLabel = (s: string) => s.replace(/"/g, "'");
					const nodeLines = bcNames.map((n, i) =>
						"    " + String.fromCharCode(65 + i) + "[\"" + escLabel(n) + "\"]"
					).join("\n");
					const edgeLines = bcNames.slice(0, -1).map((n, i) =>
						"    " + String.fromCharCode(65 + i) + " --> " + String.fromCharCode(66 + i) + " : events"
					).join("\n");
					const lastNode = bcNames.length > 1
						? "    " + String.fromCharCode(64 + bcNames.length) + " --> Downstream[\"Consumers\"]"
						: bcNames.length === 1
							? "    A[\"" + escLabel(bcNames[0]) + "\"] --> Downstream[\"Consumers\"]"
							: "";
					bcDiagram = nodeLines + "\n\n" + edgeLines + (lastNode ? "\n" + lastNode : "");
				} else {
					bcDiagram = "    A[\"Bounded Context 1\"] --> B[\"Bounded Context 2\"] : events";
				}

				const diagramContent = [
					"# System Context Diagram",
					"",
					"## Context",
					"",
					contextName,
					"",
					"## Bounded Contexts Flow",
					"",
					"```mermaid",
					"graph LR",
					bcDiagram,
					"```",
					"",
					"---",
					"",
					"*Generated from session: " + sessionId,
					"*Date: " + timestamp,
				].join("\n");

				const diagramTmp = diagramPath + ".tmp";
				fs.writeFileSync(diagramTmp, diagramContent, "utf-8");
				fs.renameSync(diagramTmp, diagramPath);

				ctx.ui.notify(
				"Architecture scaffolded: " + scaffoldModules.length + " modules, ADR-001, diagrams",
				"success",
				);

				const resultLines = [
					"## Architecture Scaffold Complete",
					"",
					"Session: " + sessionId,
					"",
					"### Generated Artifacts",
				];

				resultLines.push("", "**Module Documents** (" + modulesDir + "):");
				if (scaffoldModules.length > 0) {
					for (const mod of scaffoldModules) {
						resultLines.push("- " + mod);
					}
				} else {
					resultLines.push("- (agent should create module docs from bounded contexts)");
				}

				resultLines.push("", "**Architecture Decisions** (" + decisionsDir + "):");
				resultLines.push("- ADR-001-architecture-pattern.md");

				resultLines.push("", "**Diagrams** (" + diagramsDir + "):");
				resultLines.push("- system-context.md");

				resultLines.push("", "**Bounded Contexts Discovered**: " + bcNames.length);
				for (const bc of bcNames) {
					resultLines.push("- " + bc);
				}

				if (scaffoldError) {
					resultLines.push("", "**Note**: " + scaffoldError);
				}

				resultLines.push("", "### Next Steps");
				resultLines.push("1. Review the module docs in .pi/architecture/modules/");
				resultLines.push("2. Review ADR-001 in .pi/architecture/decisions/");
				resultLines.push("3. Review the system diagram in .pi/architecture/diagrams/");
				resultLines.push("4. Use /epic-plan --overview or /architect to plan implementation");
				resultLines.push("");
				resultLines.push("Or run through the full delivery pipeline:");
				resultLines.push("  1. /domain --validate " + sessionId + "    (validate exploration)");
				resultLines.push("  2. (architecture scaffold just completed)");
				resultLines.push("  3. guardian project create --lang <lang>   (Epic 0 - greenfield only)");
				resultLines.push("  4. /epic-plan --module <module>    (plan each module)");

				// Send the results as a follow-up message so the agent sees them
				try {
					pi.sendMessage(
						{ content: resultLines.join("\n"), display: true },
						{ deliverAs: "followUp", triggerTurn: true },
					);
				} catch (e) {
					return resultLines.join("\n");
				}
				return "Architecture scaffold dispatched. Session: " + sessionId;
			}
			if (trimmed.startsWith("--validate")) {
				const sessionId = trimmed.slice("--validate".length).trim();
				if (!sessionId) {
					ctx.ui.notify(
						"Usage: /domain --validate <session-id>",
						"error",
					);
					return "(domain command handled)";
				}

				const explorationDir = path.join(ctx.cwd, ".pi", "domain", "exploration");
				const sessionPath = path.join(explorationDir, sessionId + ".md");
				const checks: string[] = [];
				let allPassed = true;

				if (!fs.existsSync(sessionPath)) {
					checks.push("  FAIL Session not found");
					allPassed = false;
				} else {
					const content = fs.readFileSync(sessionPath, "utf-8");
					checks.push("  PASS Session file exists");
					checks.push(content.includes("## Bounded Contexts") ? "  PASS Bounded contexts section" : "  FAIL Missing bounded contexts section");
					checks.push(content.includes("## Entities") ? "  PASS Entities section" : "  FAIL Missing entities section");
					checks.push(content.includes("## Ubiquitous Language") ? "  PASS Ubiquitous language section" : "  FAIL Missing ubiquitous language section");
				}

				const header = allPassed ? "Domain Validation - All Checks Passed" : "Domain Validation - Some Checks Failed";
				ctx.ui.notify(header, allPassed ? "success" : "error");

				return header + "\n" + checks.join("\n");
			}

			// Default: show usage
			ctx.ui.notify(
				[
					"Usage:",
					'  /domain --explore "Business context description"',
					"  /domain --architect-scaffold <session-id>",
					"  /domain --validate <session-id>",
				].join("\n"),
				"info",
			);

			return [
				"Available /domain subcommands:",
				"",
				'  /domain --explore "..."',
				"    Start a DDD domain exploration — agent writes exploration.md + glossary directly",
				"",
				"  /domain --architect-scaffold <session-id>",
				"    Generate architecture directories from exploration",
				"",
				"  /domain --validate <session-id>",
				"    Validate exploration session structure",
			].join("\n");
		},
	});
}
