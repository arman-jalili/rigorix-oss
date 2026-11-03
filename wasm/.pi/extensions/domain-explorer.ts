/**
 * Canonical Reference: .pi/architecture/modules/core-libraries.md
 * Implements: Domain Explore pi extension — /domain command + domain_explore/domain_validate tools
 * Last Architecture Sync: 2026-05-31
 *
 * Pi extension providing:
 *   /domain --explore — Returns DDD analysis instructions + agent writes files directly
 *   /domain --architect-scaffold — Generate architecture directories from exploration
 *   /domain --enrich — Enrich module stubs with full DDD content
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
// @ts-ignore — typebox is provided at runtime by pi
import { Type } from "typebox";

// Strip markdown formatting (**bold**, *italic*, `code`) from a string
function stripMarkdown(text: string): string {
	return text
		.replace(/\*\*(.+?)\*\*/g, "$1")
		.replace(/\*(.+?)\*/g, "$1")
		.replace(/`(.+?)`/g, "$1")
		.trim();
}

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
function readLanguage(cwd: string): string {
	try {
		const manifestPath = path.join(cwd, "guardian-manifest.json");
		if (fs.existsSync(manifestPath)) {
			const data = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));
			if (data.language) return data.language;
		}
		const piManifestPath = path.join(cwd, ".pi", "guardian-manifest.json");
		if (fs.existsSync(piManifestPath)) {
			const data = JSON.parse(fs.readFileSync(piManifestPath, "utf-8"));
			if (data.language) return data.language;
		}
	} catch {
		// ignore
	}
	return "typescript";
}

// Prefer the PROJECT-LOCAL guardian-framework (node_modules/.bin) over any bare
// global install. A stale global on PATH (e.g. /opt/homebrew/bin) silently ran an
// old build without the implemented-module preservation guard and clobbered docs.
function guardianBin(ctx: ExtensionContext): string {
	const local = path.join(ctx.cwd, "node_modules", ".bin", "guardian-framework");
	if (fs.existsSync(local)) return JSON.stringify(local);
	return "npx --no-install guardian-framework";
}

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
			onUpdate: (update: { type?: string; message?: string; content?: unknown }) => void,
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

// True when a JSON string was cut off mid-output (unclosed structures or a
// mid-string cut) — distinguishes truncation from invalid-but-complete JSON.
function isTruncatedJsonText(text: string): boolean {
	const stack: string[] = [];
	let inString = false;
	let escaped = false;
	for (let i = 0; i < text.length; i++) {
		const ch = text[i];
		if (inString) {
			if (escaped) escaped = false;
			else if (ch === "\\") escaped = true;
			else if (ch === '"') inString = false;
			continue;
		}
		if (ch === '"') inString = true;
		else if (ch === "{" || ch === "[") stack.push(ch);
		else if (ch === "}" || ch === "]") {
			const open = stack.pop();
			if ((ch === "}" && open !== "{") || (ch === "]" && open !== "[")) return false;
		}
	}
	return inString || stack.length > 0;
}

function pickKey(item: Record<string, unknown>, keys: string[]): string {
	return keys.map((k) => String(item[k] ?? "")).join("|");
}

function mergeArrayById(
	existing: Record<string, unknown>[],
	incoming: Record<string, unknown>[],
	keys: string[],
): Record<string, unknown>[] {
	const out = new Map<string, Record<string, unknown>>();
	for (const item of existing) out.set(pickKey(item, keys), item);
	for (const item of incoming) {
		const k = pickKey(item, keys);
		const prev = out.get(k);
		if (prev) out.set(k, { ...prev, ...item });
		else out.set(k, item);
	}
	return Array.from(out.values());
}

// Merge a repeated domain_save_result call into a previously saved session so
// large domains can be saved in parts without a single oversized response.
function mergeRawJson(existing: Record<string, unknown>, incoming: Record<string, unknown>): Record<string, unknown> {
	const merged: Record<string, unknown> = { ...existing, ...incoming };
	const mergeArr = (key: string, keys: string[]) => {
		const a = Array.isArray(existing[key]) ? (existing[key] as Record<string, unknown>[]) : [];
		const b = Array.isArray(incoming[key]) ? (incoming[key] as Record<string, unknown>[]) : [];
		merged[key] = mergeArrayById(a, b, keys);
	};
	mergeArr("actors", ["name"]);
	mergeArr("functionalRequirements", ["id"]);
	mergeArr("nonFunctionalRequirements", ["id"]);
	mergeArr("assumptions", ["assumption"]);
	mergeArr("applicationServices", ["name", "context"]);
	mergeArr("domainEvents", ["name", "context"]);
	mergeArr("ubiquitousLanguage", ["term", "boundedContext"]);
	// entities dedupe by (context, name)
	const entA = Array.isArray(existing.entities) ? (existing.entities as Record<string, unknown>[]) : [];
	const entB = Array.isArray(incoming.entities) ? (incoming.entities as Record<string, unknown>[]) : [];
	merged.entities = mergeArrayById(entA, entB, ["name", "context"]);
	// bounded contexts: merge by name; union entity lists + invariants
	const bcA = Array.isArray(existing.boundedContexts) ? (existing.boundedContexts as Record<string, unknown>[]) : [];
	const bcB = Array.isArray(incoming.boundedContexts) ? (incoming.boundedContexts as Record<string, unknown>[]) : [];
	const bcMap = new Map<string, Record<string, unknown>>();
	for (const bc of [...bcA, ...bcB]) {
		const name = String(bc.name ?? "");
		if (!name) continue;
		const prev = bcMap.get(name);
		if (!prev) { bcMap.set(name, { ...bc }); continue; }
		const entities = new Set<string>([
			...(Array.isArray(prev.entities) ? prev.entities.map(String) : []),
			...(Array.isArray(bc.entities) ? bc.entities.map(String) : []),
		]);
		const invariants = Array.isArray(bc.invariants) && bc.invariants.length > 0
			? bc.invariants
			: prev.invariants;
		bcMap.set(name, {
			...prev,
			...bc,
			entities: Array.from(entities),
			invariants: invariants ?? [],
		});
	}
	merged.boundedContexts = Array.from(bcMap.values());
	// Original prompt/context is immutable once seeded — later saves (summaries,
	// edits) must never erase it.
	for (const key of ["businessContext", "originalPrompt"]) {
		const cur = existing[key];
		const v = incoming[key];
		if (cur !== undefined && String(cur).trim() !== "") merged[key] = cur;
		else if (v !== undefined && String(v).trim() !== "" && String(v).trim() !== "None") merged[key] = v;
	}
	for (const key of ["openQuestions", "aggregateRoots", "status"]) {
		const v = incoming[key];
		if (v !== undefined && String(v).trim() !== "" && String(v).trim() !== "None") merged[key] = v;
	}
	return merged;
}

// True when a module has real implementation code (not just a planned doc).
// ── Self-driving exploration driver ─────────────────────────────────────────
// After every successful domain_save_result merge the driver inspects the saved
// session and dispatches the NEXT phase as its own small agent turn. The model is
// never asked to carry the whole protocol, and a truncation anywhere only loses
// the current small batch — /domain --resume <session> re-dispatches exactly the
// next unfinished phase from persisted state (nothing is ever re-derived).

type PhaseKind = "strategic" | "requirements" | "usecases" | "context" | "done";

function nextPhaseDecision(raw: Record<string, unknown>): { kind: PhaseKind; contextName?: string } {
	const bcs = Array.isArray(raw.boundedContexts) ? (raw.boundedContexts as Record<string, unknown>[]) : [];
	if (bcs.length === 0) return { kind: "strategic" };
	const prog = (raw.protocolProgress as Record<string, unknown>) ?? {};
	if (prog.requirementsComplete !== true) return { kind: "requirements" };
	if (prog.usecasesComplete !== true) return { kind: "usecases" };
	// Per-context tactical passes for every non-supporting/generic context that is
	// not yet enriched (all strategic-listed entities present or any entity saved).
	const supporting = new Set(bcs
		.filter((bc) => /supporting|generic/i.test(String(bc.subdomainType ?? "")))
		.map((bc) => String(bc.name ?? "")));
	const saved = Array.isArray(raw.entities) ? (raw.entities as Record<string, unknown>[]) : [];
	const byContext = new Map<string, Set<string>>();
	for (const e of saved) {
		const c = String(e.context ?? "");
		const n = String(e.name ?? "");
		if (!c || !n) continue;
		if (!byContext.has(c)) byContext.set(c, new Set());
		byContext.get(c)!.add(n);
	}
	const enriched = (name: string): boolean => {
		const bc = bcs.find((x) => String(x.name ?? "") === name);
		const need = (Array.isArray(bc?.entities) ? (bc!.entities as unknown[]) : []).map(String).filter(Boolean);
		const have = byContext.get(name);
		if (need.length > 0) return need.every((n) => have?.has(n));
		return (have?.size ?? 0) > 0 || (Array.isArray(bc?.invariants) && (bc!.invariants as unknown[]).length > 0);
	};
	for (const bc of bcs) {
		const name = String(bc.name ?? "");
		if (!name || supporting.has(name) || enriched(name)) continue;
		const remaining = (Array.isArray(bc.entities) ? (bc.entities as unknown[]) : [])
			.map(String).filter((n) => !byContext.get(name)?.has(n));
		return { kind: "context", contextName: name };
	}
	return { kind: "done" };
}

function maxSeqId(items: unknown[] | undefined, prefix: string): number {
	let max = 0;
	for (const it of (items ?? []) as Record<string, unknown>[]) {
		const m = String(it.id ?? "").match(new RegExp("^" + prefix + "-(\\d+)$"));
		if (m) max = Math.max(max, Number(m[1]));
	}
	return max;
}

const DRIVER_RULES = [
	"- Reply with ONLY the small JSON for the requested step — no prose, no fences, no reasoning.",
	"- Emit AT MOST 5 items per message (fewer if any item is long). Keep the JSON well under",
	"  ~2,500 tokens so the domain_save_result call that follows always fits in the same message.",
	"- Call domain_save_result with { sessionId, responseJson: <that exact JSON> } and STOP —",
	"  nothing after the tool call. The driver sends your next step after it saves.",
].join("\n");

function buildStrategicMessage(sessionId: string, businessContext: string): string {
	return [
		"Analyze this business domain with Domain-Driven Design. Work in SMALL incremental steps —",
		"one phase per message; the driver will send each next phase after you save. This first",
		"message is the STRATEGIC PASS only.",
		"",
		"Session: " + sessionId,
		"Business Context: " + businessContext,
		"",
		"Reply with ONLY this JSON:",
		'```json',
		'{',
		'  "businessContext": "one-line summary of the domain",',
		'  "actors": [{ "name": "...", "description": "...", "interactions": "..." }],',
		'  "assumptions": [{ "assumption": "...", "impactIfWrong": "...", "mitigation": "..." }],',
		'  "boundedContexts": [',
		'    { "name": "...", "description": "...", "entities": ["..."],',
		'      "subdomainType": "core|supporting|generic", "subdomainRationale": "one line" }',
		'  ],',
		'  "contextMap": [{ "from": "A", "to": "B", "relationship": "partnership|shared-kernel|conformist|anticorruption-layer|open-host", "note": "..." }],',
		'  "openQuestions": "..."',
		'}',
		'```',
		"",
		DRIVER_RULES,
	].join("\n");
}

function buildRequirementsMessage(raw: Record<string, unknown>, sessionId: string): string {
	const fr = Array.isArray(raw.functionalRequirements) ? (raw.functionalRequirements as Record<string, unknown>[]) : [];
	const nfr = Array.isArray(raw.nonFunctionalRequirements) ? (raw.nonFunctionalRequirements as Record<string, unknown>[]) : [];
	const nextFr = maxSeqId(fr, "FR") + 1;
	const nextNfr = maxSeqId(nfr, "NFR") + 1;
	return [
		"Session " + sessionId + " — REQUIREMENTS PASS (batch). Saved so far: " +
			fr.length + " functional, " + nfr.length + " non-functional.",
		"Produce the NEXT contiguous batch: up to 5 functional requirements starting at FR-" +
			String(nextFr).padStart(3, "0") + (fr.length === 0 ? "" : " (continue from the last saved id)") +
			" and/or up to 5 non-functional requirements starting at NFR-" + String(nextNfr).padStart(3, "0") + ".",
		"Only include the type you still have left to deliver.",
		"",
		"Reply with ONLY this JSON:",
		'```json',
		'{',
		'  "functionalRequirements": [',
		'    { "id": "FR-001", "requirement": "The system shall...", "priority": "critical|high|medium|low", "boundedContext": "ContextName" }',
		'  ],',
		'  "nonFunctionalRequirements": [',
		'    { "id": "NFR-001", "requirement": "The system shall...", "category": "performance|security|scalability|availability|maintainability", "target": "measurable target" }',
		'  ]',
		'}',
		'```',
		"On your FINAL requirements batch add \"requirementsComplete\": true to the JSON",
		"(omit both arrays if a type is done and only send the flag).",
		"",
		DRIVER_RULES,
	].join("\n");
}

function buildUsecasesMessage(sessionId: string): string {
	return [
		"Session " + sessionId + " — USE-CASES PASS (batch). Produce up to 4 application services for the",
		"core contexts (name the owning context exactly as in the strategic pass).",
		"",
		"Reply with ONLY this JSON:",
		'```json',
		'{',
		'  "applicationServices": [',
		'    { "name": "ApproveRequest", "context": "ApprovalWorkflow", "useCase": "what this application service orchestrates",',
		'      "commands": ["verify intent", "consume"], "aggregates": ["ApprovalTicket"], "mapsToFr": "FR-00x" }',
		'  ]',
		'}',
		'```',
		"On your FINAL use-cases batch add \"usecasesComplete\": true to the JSON.",
		"",
		DRIVER_RULES,
	].join("\n");
}

function buildContextMessage(raw: Record<string, unknown>, sessionId: string, contextName: string): string {
	const bc = (Array.isArray(raw.boundedContexts) ? (raw.boundedContexts as Record<string, unknown>[]) : [])
		.find((x) => String(x.name ?? "") === contextName);
	const need = (Array.isArray(bc?.entities) ? (bc!.entities as unknown[]) : []).map(String).filter(Boolean);
	const done = Array.isArray(raw.entities) ? (raw.entities as Record<string, unknown>[])
		.filter((e) => String(e.context ?? "") === contextName).map((e) => String(e.name ?? "")) : [];
	const remaining = need.filter((n) => !done.includes(n));
	return [
		"Session " + sessionId + " — PER-CONTEXT PASS for \"" + contextName + "\"" +
			(remaining.length > 0 ? " (still needed from strategic list: " + remaining.join(", ") + ")" : "") + ".",
		"Enrich this ONE context: entities with entity-vs-VO justification, domain events with triage,",
		"invariants, and this context's ubiquitous language. Up to 6 entities per message — if the",
		"context has more, the driver will ask you to continue this same context.",
		"",
		"Reply with ONLY this JSON:",
		'```json',
		'{',
		'  "boundedContexts": [{ "name": "' + contextName + '", "description": "...", "entities": ["Entity1"],',
		'    "invariants": [{ "rule": "...", "illegalStatePrevented": "..." }] }],',
		'  "entities": [',
		'    { "name": "Entity1", "context": "' + contextName + '", "type": "entity|value-object|aggregate-root",',
		'      "description": "...",',
		'      "justification": { "identityPersists": true, "immutable": false, "equalByState": false } }',
		'  ],',
		'  "domainEvents": [',
		'    { "name": "Event", "context": "' + contextName + '", "description": "...", "triggeredBy": "...",',
		'      "triage": { "domainExpertCares": true, "crossesAggregateBoundary": false, "needsAudit": false } }',
		'  ],',
		'  "ubiquitousLanguage": [{ "term": "...", "definition": "...", "boundedContext": "' + contextName + '", "aliases": [], "examples": "" }]',
		'}',
		'```',
		"",
		DRIVER_RULES,
	].join("\n");
}

function buildPhaseMessage(kind: PhaseKind, raw: Record<string, unknown>, sessionId: string, contextName?: string): string {
	if (kind === "strategic") return buildStrategicMessage(sessionId, String(raw.businessContext ?? ""));
	if (kind === "requirements") return buildRequirementsMessage(raw, sessionId);
	if (kind === "usecases") return buildUsecasesMessage(sessionId);
	if (kind === "context") return buildContextMessage(raw, sessionId, contextName ?? "");
	return "";
}

function moduleHasCode(cwd: string, moduleName: string): boolean {
	const sanitized = (moduleName || "").toLowerCase().replace(/[^a-z0-9]/g, "");
	if (!sanitized) return false;
	const roots = ["src", "crates", "packages", "internal", "lib", "app", "modules"];
	const codeFile = (n: string): boolean => /.*\.(java|kt|ts|tsx|js|jsx|rs|py|go)$/.test(n) && !/test|spec/i.test(n);
	const walk = (dir: string, depth: number): boolean => {
		if (depth > 6) return false;
		let entries;
		try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return false; }
		for (const e of entries) {
			if (["node_modules", "target", "dist", ".git"].includes(e.name)) continue;
			const full = path.join(dir, e.name);
			if (e.isFile() && codeFile(e.name)) return true;
			if (e.isDirectory() && (e.name === sanitized || walk(full, depth + 1))) {
				// entered the module dir: any code file below counts
				if (e.name === sanitized && hasCodeBelow(full, depth)) return true;
			}
		}
		return false;
	};
	const hasCodeBelow = (dir: string, depth: number): boolean => {
		let entries;
		try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return false; }
		for (const e of entries) {
			if (["node_modules", "target", "dist"].includes(e.name)) continue;
			const full = path.join(dir, e.name);
			if (e.isFile() && codeFile(e.name)) return true;
			if (e.isDirectory() && depth < 6 && hasCodeBelow(full, depth + 1)) return true;
		}
		return false;
	};
	for (const r of roots) {
		const base = path.join(cwd, r);
		if (fs.existsSync(base) && walk(base, 0)) return true;
	}
	return false;
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
			"Incremental: repeated calls merge into the session, so large domains can be " +
			"delivered in parts (boundedContexts first, then per-context enrichments) instead of " +
			"one oversized response that truncates. Writes exploration.md, session file and .raw.json; " +
			"updates ubiquitous-language.md.",
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
			catch (e) {
				if (isTruncatedJsonText(cleaned)) {
					return { content: [{ type: "text" as const, text:
						"ERROR: domain JSON was truncated before completion. Never resend a partial model as final. " +
						"Options: (1) retry with the full completed JSON; (2) for large domains save IN PARTS — " +
						"domain_save_result merges: send boundedContexts first, then per-context enrichments " +
						"(entities/events/UL/invariants) across several calls; (3) reduce model output strain by " +
						"phasing the exploration." }] };
				}
				return { content: [{ type: "text" as const, text: "ERROR: Failed to parse JSON: " + String(e) }] };
			}

			// Phase-completion flags ride along on the model's JSON but are not part of
			// the persisted schema — pull them off before merging so they never pollute
			// the raw session, then fold them into protocolProgress below.
			const flagsRc = parsedJson.requirementsComplete === true;
			const flagsUc = parsedJson.usecasesComplete === true;
			delete parsedJson.requirementsComplete;
			delete parsedJson.usecasesComplete;

			// Incremental: merge into any previously saved structured session
			// (<session>.raw.json) so large domains can be saved in parts.
			const rawPath = path.join(ctx.cwd, ".pi", "domain", "exploration", sessionId + ".raw.json");
			if (fs.existsSync(rawPath)) {
				try {
					const existing = JSON.parse(fs.readFileSync(rawPath, "utf-8")) as Record<string, unknown>;
					parsedJson = mergeRawJson(existing, parsedJson);
				} catch { /* ignore unreadable prior state */ }
			}
			// Normalize identity/state so every saved session is self-describing
			// even when saves arrive before/without the explore seed.
			parsedJson.sessionId = parsedJson.sessionId ?? sessionId;
			parsedJson.status = String(parsedJson.status ?? "draft");
			parsedJson.createdAt = parsedJson.createdAt ?? new Date().toISOString();

			// Self-driving protocol progress. The driver asks the model ONE phase at a
			// time; these markers say which phases are finished. Stored separately so
			// the model schema stays clean and id-keyed merges never disturb them.
			const prevRawForProg = (() => {
				try { return JSON.parse(fs.readFileSync(rawPath, "utf-8")) as Record<string, unknown>; }
				catch { return {}; }
			})();
			const prevProg = (prevRawForProg.protocolProgress as Record<string, unknown>) ?? {};
			parsedJson.protocolProgress = {
				requirementsComplete: prevProg.requirementsComplete === true || flagsRc,
				usecasesComplete: prevProg.usecasesComplete === true || flagsUc,
			};

			// Derive aggregate roots from the entities table — never let the
			// rendered "## Aggregate Roots" contradict entities marked
			// aggregate-root (the protocol passes don't carry this field).
			const aggregateRootNames = (Array.isArray(parsedJson.entities) ? parsedJson.entities : [])
				.filter((e) => String((e as Record<string, unknown>).type ?? "") === "aggregate-root")
				.map((e) => String((e as Record<string, unknown>).name ?? ""))
				.filter(Boolean);
			if (aggregateRootNames.length > 0) {
				parsedJson.aggregateRoots = aggregateRootNames.join(", ");
			} else if (!String(parsedJson.aggregateRoots ?? "").trim()) {
				parsedJson.aggregateRoots = "None identified yet.";
			}

			// Backfill business context from the explore stub if the agent never
			// supplied it (front matter business_context survives across saves).
			if (String(parsedJson.businessContext ?? "").trim() === "") {
				const stubPath = path.join(ctx.cwd, ".pi", "domain", "exploration", sessionId + ".md");
				if (fs.existsSync(stubPath)) {
					try {
						const stub = fs.readFileSync(stubPath, "utf-8");
						const m = stub.match(/^business_context:\s*"((?:[^"\\]|\\.)*)"/m);
						if (m) parsedJson.businessContext = m[1].replace(/\\"/g, '"');
					} catch { /* ignore */ }
				}
			}


			const now = new Date().toISOString().split("T")[0];
			const explorationDir = path.join(ctx.cwd, ".pi", "domain", "exploration");
			fs.mkdirSync(explorationDir, { recursive: true });

			function toRow(arr: unknown[], fields: string[]): string {
				if (!Array.isArray(arr) || arr.length === 0) return "None identified yet.";
				return (arr as Record<string, unknown>[]).map((item) => {
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
			const ucs = toRow(parsedJson.applicationServices as unknown[], ["name", "context", "useCase", "commands"]);
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
				"", "---", "", "## Use Cases (Application Services)",
				"", "| Use Case | Context | Description | Commands / Collaborators |", "|-----------|---------|-------------|---------------------------|", ucs,
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

			// Persist the merged structured result (enables incremental saves +
			// downstream depth validators that read .raw.json)
			const rawTmp = rawPath + ".tmp";
			fs.writeFileSync(rawTmp, JSON.stringify(parsedJson, null, 2), "utf-8");
			fs.renameSync(rawTmp, rawPath);

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

			onUpdate?.({ content: [{ type: "text", text: "Domain exploration saved for session: " + sessionId }] });

			// Self-driving: after every successful save, dispatch the next phase as a
			// fresh small agent turn (or tell the model exploration is complete). If a
			// phase needs more batches (requirements >5, context >6 entities) the same
			// decision re-issues that phase with progress hints — never a reset.
			const decision = nextPhaseDecision(parsedJson);
			const savedNote = "Domain exploration saved for " + sessionId + ".";
			if (decision.kind === "done") {
				return { content: [{ type: "text" as const, text: savedNote + " Exploration content complete — run /domain --validate " + sessionId }] };
			}
			const label = decision.kind === "context" ? "context " + (decision.contextName ?? "") : decision.kind;
			const nextMsg = buildPhaseMessage(decision.kind, parsedJson, sessionId, decision.contextName);
			try {
				pi.sendMessage(
					{ content: nextMsg, display: true },
					{ deliverAs: "followUp", triggerTurn: true },
				);
				return { content: [{ type: "text" as const, text: savedNote + " Next phase dispatched: " + label }] };
			} catch {
				return { content: [{ type: "text" as const, text: savedNote + "\n\nNEXT PHASE (" + label + "):\n" + nextMsg }] };
			}
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

			// 4b. ADR-008 modeling-depth rules (deterministic, over raw session JSON)
			const depthRawPath = sessionPath.replace(/\.md$/, "") + ".raw.json";
			const depthScript = path.join(ctx.cwd, ".pi", "scripts", "validate-exploration-depth.sh");
			if (fs.existsSync(depthRawPath) && fs.existsSync(depthScript)) {
				try {
					const depthResult = await shellExec(ctx, "bash " + depthScript + " " + depthRawPath);
					const lines = depthResult.stdout.trim().split("\n").filter((l) => l.trim() !== "");
					const violations = lines.filter((l) => l.includes("[") && l.includes("] error:"));
					addCheck(
						"Modeling depth rules (ADR-008)",
						depthResult.exitCode === 0,
						violations.length > 0 ? violations.join(" | ") : lines.slice(-1)[0] || "",
					);
				} catch (err) {
					addCheck("Modeling depth rules (ADR-008)", false, "Error: " + String(err));
				}
			} else {
				addCheck("Modeling depth rules (ADR-008) - skipped", true, "no session raw.json");
			}

			// 3b. Completeness advisory: flag empty core sections + which protocol
			// pass fills them (empty by construction must not look like success).
			const rawPath = path.join(ctx.cwd, ".pi", "domain", "exploration", sessionId + ".raw.json");
			if (fs.existsSync(rawPath)) {
				try {
					const raw = JSON.parse(fs.readFileSync(rawPath, "utf-8")) as Record<string, unknown>;
					const sections: Array<[string, string, string]> = [
						["boundedContexts", "bounded contexts", "strategic pass (step 1)"],
						["functionalRequirements", "functional requirements", "requirements pass (step 1b)"],
						["nonFunctionalRequirements", "non-functional requirements", "requirements pass (step 1b)"],
						["entities", "entities", "per-context pass (step 2)"],
						["domainEvents", "domain events", "per-context pass (step 2)"],
						["ubiquitousLanguage", "ubiquitous-language terms", "per-context pass (step 2)"],
						["applicationServices", "use cases / application services", "use-cases pass (step 1c)"],
					];
					const empty = sections.filter(([key]) => !Array.isArray(raw[key]) || (raw[key] as unknown[]).length === 0);
					if (empty.length > 0) {
						const hint = empty
							.map(([, label, pass]) => `${label} (${pass})`)
							.join("; ");
						checks.push("  NOTE empty sections — run the matching pass: " + hint);
					}
				} catch { /* ignore */ }
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

				// Seed the structured session with the ORIGINAL prompt so it is never
				// lost: businessContext/originalPrompt carry the full business text and
				// survive every domain_save_result merge (protected in mergeRawJson).
				const seedRaw = {
					sessionId,
					businessContext: sanitized,
					originalPrompt: sanitized,
					status: "draft",
					createdAt: new Date().toISOString(),
				};
				const seedPath = path.join(explorationDir, sessionId + ".raw.json");
				const seedTmp = seedPath + ".tmp";
				fs.writeFileSync(seedTmp, JSON.stringify(seedRaw, null, 2), "utf-8");
				fs.renameSync(seedTmp, seedPath);

				ctx.ui.notify(
					"Domain analysis requested: " + sessionId,
					"success",
				);

				// Inject the DDD analysis prompt as a follow-up message to the agent.
				// Using sendMessage with triggerTurn=true causes the agent to process
				// this as a new conversation turn, not as a command response.
				const analysisPrompt = buildStrategicMessage(sessionId, sanitized);
				

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

			// /domain --resume <session-id> — re-dispatch the next unfinished phase from
			// persisted state. Truncation anywhere in the protocol is recoverable with
			// this: it never asks the model to reproduce saved content.
			if (trimmed.startsWith("--resume")) {
				const sessionId = trimmed.slice("--resume".length).trim().split(/\s+/)[0] || "";
				if (!sessionId) {
					ctx.ui.notify("Usage: /domain --resume <session-id>", "error");
					return "(domain command handled)";
				}
				const explorationDir = path.join(ctx.cwd, ".pi", "domain", "exploration");
				const rawPath = path.join(explorationDir, sessionId + ".raw.json");
				if (!fs.existsSync(rawPath)) {
					ctx.ui.notify("Session not found: " + sessionId + ". Run /domain --explore first.", "error");
					return "(domain command handled)";
				}
				let raw: Record<string, unknown> = {};
				try { raw = JSON.parse(fs.readFileSync(rawPath, "utf-8")) as Record<string, unknown>; }
				catch { ctx.ui.notify("Session raw state unreadable for " + sessionId, "error"); return "(domain command handled)"; }
				const decision = nextPhaseDecision(raw);
				if (decision.kind === "done") {
					ctx.ui.notify("Exploration content is complete — run /domain --validate " + sessionId, "info");
					return "(domain command handled)";
				}
				const label = decision.kind === "context" ? "context " + decision.contextName : decision.kind;
				ctx.ui.notify("Resumed " + sessionId + " at phase: " + label, "success");
				const msg = buildPhaseMessage(decision.kind, raw, sessionId, decision.contextName);
				try {
					pi.sendMessage(
						{ content: msg, display: true },
						{ deliverAs: "followUp", triggerTurn: true },
					);
					return "Resumed " + sessionId + " at phase: " + label;
				} catch {
					return msg;
				}
			}

			// /domain --architect-scaffold <session-id>
			// /domain --architect-scaffold <session-id>
			if (trimmed.startsWith("--architect-scaffold")) {
				const rest = trimmed.slice("--architect-scaffold".length).trim();
				const forceScaffold = rest.includes("--force");
				const sessionId = rest.split(/\s+/)[0] || "";
				if (!sessionId) {
					ctx.ui.notify(
						"Usage: /domain --architect-scaffold <session-id> [--force]",
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
				// stripMarkdown moved to module level to avoid ES5 strict mode issue

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
							bcNames.push(stripMarkdown(cells[0]));
						}
					}
				}

				// Step 1: Call guardian CLI to scaffold module docs per bounded context
				let scaffoldModules: string[] = [];
				let scaffoldWarnings: string[] = [];
				let scaffoldError = "";

				try {
					const cmd = guardianBin(ctx) + " domain scaffold " + sessionId + (forceScaffold ? " --force" : "") + " 2>&1";
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

				// Step 2: Refresh ADR-001 only when it does not exist yet (or --force).
				// Never clobber an existing ADR that documents implemented architecture.
				const adrPath = path.join(decisionsDir, "ADR-001-architecture-pattern.md");
				if (fs.existsSync(adrPath) && !forceScaffold) {
					ctx.ui.notify("Existing ADR-001 preserved — module docs scaffolded (use --force to refresh ADR)", "info");
					return "(domain command handled)";
				}
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
				// Each line is a separate array element to guarantee newline separation
				const escLabel = (s: string) => s.replace(/"/g, "'");
				const diagramLines: string[] = [];
				if (bcNames.length > 0) {
					for (let i = 0; i < bcNames.length; i++) {
						diagramLines.push("    " + String.fromCharCode(65 + i) + "[\"" + escLabel(bcNames[i]) + "\"]");
					}
					// blank line separator between nodes and edges
					diagramLines.push("");
					for (let i = 0; i < bcNames.length - 1; i++) {
						diagramLines.push("    " + String.fromCharCode(65 + i) + " --> " + String.fromCharCode(66 + i) + " : events");
					}
					if (bcNames.length > 1) {
						diagramLines.push("    " + String.fromCharCode(64 + bcNames.length) + " --> Downstream[\"Consumers\"]");
					} else {
						diagramLines.push("    A[\"" + escLabel(bcNames[0]) + "\"] --> Downstream[\"Consumers\"]");
					}
				} else {
					diagramLines.push("    A[\"Bounded Context 1\"] --> B[\"Bounded Context 2\"] : events");
				}
				const bcDiagram = diagramLines.join("\n");

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
				resultLines.push("1. Review ADR-001 in .pi/architecture/decisions/");
				resultLines.push("2. Review the system diagram in .pi/architecture/diagrams/");
				resultLines.push("3. Use /epic-plan --overview or /architect to plan implementation");
				resultLines.push("");
				resultLines.push("To enrich stubs with full DDD content: /domain --enrich " + sessionId);

				ctx.ui.notify("Architecture scaffold complete", "success");
				return resultLines.join("\n");
			}
			if (trimmed.startsWith("--enrich")) {
				const restE = trimmed.slice("--enrich".length).trim();
				const forceEnrich = restE.includes("--force");
				const sessionId = restE.split(/\s+/)[0] || "";
				if (!sessionId) {
					ctx.ui.notify("Usage: /domain --enrich <session-id> [--force]", "error");
					return "(domain command handled)";
				}

				const explorationDir = path.join(ctx.cwd, ".pi", "domain", "exploration");
				const sessionPath = path.join(explorationDir, sessionId + ".md");
				const modulesDir = path.join(ctx.cwd, ".pi", "architecture", "modules");

				if (!fs.existsSync(sessionPath)) {
					ctx.ui.notify("Session not found: " + sessionId + ". Run /domain --explore first.", "error");
					return "(domain command handled)";
				}

				// Enrichment is SESSION-SCOPED: only module docs that belong to THIS
				// session's bounded contexts may be touched, and existing module docs
				// are preserved unless --force. Prevents clobbering other streams.
				const allModules = fs.readdirSync(modulesDir).filter(f => f.endsWith(".md") && f !== "module-template.md");
				// Match a bounded-context name to its module doc by NORMALIZED name
				// (ignoring case and all separators) — camelCase contexts ("IntentCapture")
				// scaffold to hyphenated files ("intent-capture.md"); naive kebab of a
				// camelCase name yields "intentcapture.md" and matches nothing.
				const norm = (n: string) => n.toLowerCase().replace(/[^a-z0-9]/g, "");
				const moduleFileFor = (name: string): string => {
					const wanted = norm(name);
					const hit = allModules.find((f) => norm(f.replace(/\.md$/, "")) === wanted);
					if (hit) return hit;
					const kebabbed = name.trim().replace(/([a-z0-9])([A-Z])/g, "$1-$2")
						.replace(/[\s_]+/g, "-").toLowerCase().replace(/[^a-z0-9-]+/g, "").replace(/-+/g, "-").replace(/(^-|-$)/g, "");
					return kebabbed ? kebabbed + ".md" : "";
				};
				let sessionModules: string[] = [];
				const seenFiles = new Set<string>();
				const addModule = (n: string) => {
					if (!n) return;
					const f = moduleFileFor(n);
					if (f && !seenFiles.has(f)) { seenFiles.add(f); sessionModules.push(f); }
				};
				const sRawPath = path.join(explorationDir, sessionId + ".raw.json");
				if (fs.existsSync(sRawPath)) {
					try {
						const rawS = JSON.parse(fs.readFileSync(sRawPath, "utf-8")) as Record<string, unknown>;
						const bcs = Array.isArray(rawS.boundedContexts) ? (rawS.boundedContexts as Record<string, unknown>[]) : [];
						for (const bc of bcs) addModule(String(bc.name ?? ""));
					} catch { /* ignore */ }
				}
				if (sessionModules.length === 0) {
					// fallback: session md bounded-contexts table
					const md = fs.existsSync(sessionPath) ? fs.readFileSync(sessionPath, "utf-8") : "";
					const m = md.match(/## Bounded Contexts[\s\S]*?\n((?:\|.*\n)+?)(?=\n##|$)/);
					if (m) {
						for (const line of m[1].split("\n")) {
							const cell = line.split("|").map((c) => c.trim()).filter(Boolean)[0];
							if (cell) addModule(cell);
						}
					}
				}
				const preserved: string[] = [];
				const moduleFiles = sessionModules.filter((f) => {
					const exists = fs.existsSync(path.join(modulesDir, f));
					const implemented = moduleHasCode(ctx.cwd, f.replace(/\.md$/, ""));
					if (exists && implemented && !forceEnrich) {
						preserved.push(f);
						return false;
					}
					return true;
				});
				if (preserved.length > 0) {
					ctx.ui.notify("Preserving existing module docs (use --force to enrich): " + preserved.join(", "), "info");
				}
				if (moduleFiles.length === 0) {
					ctx.ui.notify("No new module docs for this session. Existing docs preserved — no enrich needed (or pass --force).", "error");
					return "(domain command handled)";
				}

				ctx.ui.notify("Enriching " + moduleFiles.length + " module docs from session " + sessionId, "info");
				const lang = readLanguage(ctx.cwd);

				let servicesCatalog = "(no catalog available)";
				try {
					const svcRaw = JSON.parse(fs.readFileSync(sRawPath, "utf-8")) as Record<string, unknown>;
					const svcAll = Array.isArray(svcRaw.applicationServices) ? (svcRaw.applicationServices as Record<string, unknown>[]) : [];
					const svcBcs = Array.isArray(svcRaw.boundedContexts) ? (svcRaw.boundedContexts as Record<string, unknown>[]) : [];
					const ctxFor = (file: string): string => {
						const wanted = norm(file.replace(/\.md$/, ""));
						const bc = svcBcs.find((b) => norm(String(b.name ?? "")) === wanted);
						return bc ? String(bc.name ?? "") : "";
					};
					const chunks = moduleFiles.map((f) => {
						const ctx = ctxFor(f);
						const svcs = svcAll.filter((sv) => String(sv.context ?? "") === ctx);
						if (!ctx || svcs.length === 0) return "";
						const lines = svcs.map((sv) => {
							const extra = [String(sv.useCase ?? ""), sv.mapsToFr ? "mapsToFr: " + String(sv.mapsToFr) : ""]
								.filter(Boolean).join(" — ");
							return `- \u0060${String(sv.name ?? "")}\u0060 : ${extra}`;
						});
						return "### " + ctx + "  (module doc: " + f + ")\n" + lines.join("\n");
					}).filter(Boolean);
					if (chunks.length > 0) servicesCatalog = chunks.join("\n\n");
				} catch { /* leave fallback text */ }

				const enrichPrompt = [
					"## Task: Complete Architecture Package",
					"",
					"The scaffold generated basic stubs. Your task is to produce a **complete, interconnected architecture package**",
					"that an implementation agent can build from.",
					"",
					"### Application-services catalog (AUTHORITATIVE — from the exploration use-cases pass)",
					"",
					"The Application Layer section of EVERY module doc you enrich must list exactly these",
					"application services for that module's bounded context — EXACT names, use-case, and",
					"FR mapping. Do NOT rename, drop, or substitute services, and do not invent services",
					"that replace them. Extra controllers/adapters/query helpers may be added BELOW the",
					"catalog, but every catalog entry must appear verbatim:",
					"",
					servicesCatalog,
					"",
					"**YOU MUST FOLLOW THESE 3 STEPS IN ORDER. Do not skip steps or reorder them.**",
					"",
					"### Source Data",
					"",
					"- Domain exploration: `.pi/domain/exploration/" + sessionId + ".md`",
					"- Ubiquitous Language: `.pi/domain/ubiquitous-language.md`",
					"- Current ADR-001: `.pi/architecture/decisions/ADR-001-architecture-pattern.md`",
					"- System diagram: `.pi/architecture/diagrams/system-context.md`",
					"- Module stubs: `.pi/architecture/modules/` (" + moduleFiles.length + " files)",
					"- ADR-008 modeling depth: `.pi/architecture/decisions/ADR-008-proportional-modeling-depth.md`",
					"- Structured session JSON (depth fields): `.pi/domain/exploration/" + sessionId + ".raw.json`",
					"",
					"### STEP 0: ADR-008 Modeling Depth (read first, applies everywhere)",
				"",
				"Read the `**Modeling Depth:**` header of EVERY module doc plus the structured",
				"session JSON (`.raw.json`). Enrich PROPORTIONALLY — never rewrite the depth header,",
				"never delete an existing `Modeling Depth Note (ADR-008)` section, and never invent",
				"modeling the exploration did not capture:",
				"",
				"- **tactical** (core subdomain): full DDD enrichment — aggregates as consistency",
				"  boundaries with the exploration invariants (`rule` + illegalStatePrevented),",
				"  behavior methods not setters, repository interface owned by the domain layer,",
				"  value objects with entity-vs-VO justification, triaged domain events with payloads,",
				"  and explicit domain / application / infrastructure service classification.",
				"- **service** (supporting subdomain): enrich to a Service + Persistence Contract.",
				"  Do NOT expand aggregates, value objects, domain events, or API surfaces beyond",
				"  what the exploration captured.",
				"- **adapter** (generic subdomain): enrich to an Interface + Anti-Corruption Layer —",
				"  the narrow interface the domain consumes, the vendor/OSS choice, the ACL",
				"  translation and its context-map edge. NO domain entities or aggregates.",
				"",
				"Language judgment: for Java/Spring modules apply the decision tables in the",
				"`java-spring-enterprise-codegen` skill section 10 (entity-vs-VO, service classifier,",
				"event triage, transaction placement — `@Transactional` only in application services).",
				"Diagrams and section breadth follow each module's depth (full module-level diagram",
				"set only for tactical; service = data-flow + sequence; adapter = ACL/integration).",
				"",
				"### STEP 1: Generate All ADRs",
					"",
					"**Do this first, before touching any module docs.** Module enrichment needs ADR references.",
					"",
					"Create these ADR files in `.pi/architecture/decisions/`:",
					"",
					"- ADR-001: Architecture Pattern (DDD Modular Monolith) — review and update if needed",
					"- ADR-002: Data Storage Strategy (Postgres + SQLx per context)",
					"- ADR-003: Cross-Context Communication (Domain Events)",
					"- ADR-004: API Design (REST + JSON, versioning strategy)",
					"- ADR-005: Authentication & Authorization (API Keys + Scopes)",
					"- ADR-006: Cost Tracking & Usage Metering",
					"- ADR-007: Compliance Engine Architecture",
					"",
					"Each ADR file must follow the format: **Status, Date, Context, Decision, Consequences, Alternatives**",
					"Stop after generating ALL ADR files before proceeding.",
					"",
					"### STEP 2: Generate Diagrams",
					"",
					"**After all ADRs are created, generate diagrams.**",
					"",
					"#### System-Level (in `.pi/architecture/diagrams/`)",
					"",
					"1. Update `system-context.md` with clean names",
					"2. Create `event-flow.md` — Mermaid flowchart showing domain events between all bounded contexts",
					"",
					"#### Module-Level (embedded in each module doc)",
					"",
					"In EACH module doc under `## Diagrams`, add these 4 mermaid diagrams:",
					"",
					"1. **Data Flow** — flowchart: inputs → processing → outputs → storage",
					"2. **Entity Relationship** — classDiagram: aggregates, entities, VOs, fields, relationships",
					"3. **Aggregate State** — stateDiagram-v2: states and transitions for each aggregate root",
					"4. **Key Use Case Sequence** — sequenceDiagram: Actor → Interface → App → Domain → Infra",
					"",
					"Stop after creating diagrams before proceeding.",
					"",
					"### STEP 3: Enrich Each Module Doc",
					"",
					"**After ADRs and diagrams are done, enhance each module doc.**",
					"",
					"For each file in `.pi/architecture/modules/*.md`, add:",
					"",
					"1. **## Related ADRs** — Links to ADR files with descriptions of relevance",
					"2. **## Diagrams** — Embed the 4 mermaid diagrams from Step 2",
					"3. **## Acceptance Criteria** — Add a table row for each component: | # | Component | Criterion | Verify In | with specific, testable acceptance criteria",
					"5. **## Components** — depth-aware: for tactical modules expand aggregates with " + lang + " stubs, methods, exploration invariants, VO justifications and a domain/-owned repository interface; for service/adapter modules keep only what the Modeling Depth Note permits (no invented aggregates/VOs). Always keep `status:` and `depends:` markers",
					"6. **Domain Events** (tactical modules only) — Table: Event, Description, Trigger, Payload, Published By; include ADR-008 triage flags (domain expert cares / crosses boundary / audit trail)",
					"7. **API Endpoints** — Table: Method, Path, Handler, Input, Output, Auth",
					"8. **Ubiquitous Language** — Terms for this context from `.pi/domain/ubiquitous-language.md`",
					"9. **Dependencies** — Depends On + Used By",
					"10. **Implementation Sequence** — Ordered build list",
					"11. **Patterns & OOP Mapping** — for every tactical aggregate and application",
					"    service, state the design pattern(s) it uses + a one-line rationale:",
					"    Strategy (when a rule/algorithm has 2+ variants — fee/tax/gate policies),",
					"    Factory (ONLY for complex aggregate creation), Specification (combinable",
					"    business rules), Observer/domain events (cross-aggregate flows), record/DTO",
					"    & value objects (immutables, @Embeddable). OOP: no anemic aggregates, no",
					"    setters on roots, tell-don't-ask, CQS. Do NOT abstract speculatively — only",
					"    introduce an interface/strategy when 2+ variants are known (curriculum rule).",
					"    Ensure code stubs reflect the chosen patterns.",
					"",
					"### STEP 4: Generate Implementation Roadmap",
					"",
					"**After all module docs are enriched, generate the implementation roadmap.**",
					"",
					"Read each module doc's dependency section and create `.pi/architecture/implementation-roadmap.md`",
					"with `## Phase N: Name (Days N-N)` sections, each having:",
					"- Goal, Modules table, Dependencies, Database Migrations, Acceptance Criteria",
					"",
					"Organize phases by dependency chain:",
					"- Phase 0: Foundation (no upstream deps)",
					"- Phase 1: Core services depending on Phase 0",
					"- Phase 2+: Following the dependency chain",
					"",
					"Also add: Dependency Graph, Effort Estimates table, Key Milestones",
					"",
					"### STEP 5: Fill Architecture CHANGELOG",
					"",
					"Read `.pi/architecture/CHANGELOG.md` and add an entry for each enriched module.",
					"Each entry should note: Module, what was defined (aggregates, events, endpoints),",
					"and update the Sync Status table at the bottom.",
					"",
					"### Scope Rules",
					"",
					"- CREATE new ADRs in `.pi/architecture/decisions/`",
					"- CREATE new diagram md files in `.pi/architecture/diagrams/`",
					"- CREATE `.pi/architecture/implementation-roadmap.md` with phased plan",
					"- EDIT `.pi/architecture/CHANGELOG.md` — add entries for each module",
					"- EDIT existing module docs in `.pi/architecture/modules/`",
					"- Do NOT create any source code files",
					"- Use " + lang + " syntax for code stubs",
					"- Use Mermaid for all diagrams",
				].join("\n");

				try {
					pi.sendMessage(
						{ content: enrichPrompt, display: true },
						{ deliverAs: "followUp", triggerTurn: true },
					);
				} catch (e) {
					return enrichPrompt;
				}
				ctx.ui.notify("Enrich task dispatched to agent for " + moduleFiles.length + " modules", "success");
				return "Enrich task dispatched. Session: " + sessionId;
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
				const modulesDir = path.join(ctx.cwd, ".pi", "architecture", "modules");
				const norm = (n: string) => n.toLowerCase().replace(/[^a-z0-9]/g, "");
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

								// Application-services drift gate (root cause: use-cases stopped at the
				// exploration layer). Every catalog service must be present in its module
				// doc's Application Layer catalog block; Implemented rows must resolve to a
				// real seam class in src; Planned/Seam rows must carry a roadmap/phase
				// pointer. Catches doc gaps (e.g. evidence-export had no catalog block) and
				// paper claims (doc says Implemented, code has no such seam).
				const rawVPath = sessionPath.replace(/\.md$/, ".raw.json");
				if (fs.existsSync(rawVPath)) {
					try {
						const rawV = JSON.parse(fs.readFileSync(rawVPath, "utf-8")) as Record<string, unknown>;
						const svcV = Array.isArray(rawV.applicationServices) ? (rawV.applicationServices as Record<string, unknown>[]) : [];
						if (svcV.length > 0) {
							const vModules = fs.readdirSync(modulesDir).filter((f) => f.endsWith(".md") && f !== "module-template.md");
							const typeExists = (typeName: string): boolean => {
								const srcBase = path.join(ctx.cwd, "src");
								if (!fs.existsSync(srcBase)) return false;
								const target = typeName.replace(/[^A-Za-z0-9_]/g, "");
								let found = false;
								const walk = (dir: string, dep: number): void => {
									if (found || dep > 9) return;
									let entries;
									try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
									for (const e of entries) {
										if (e.name === "node_modules" || e.name === "target" || e.name === ".git" || e.name === "dist") continue;
										const full = path.join(dir, e.name);
										if (e.isFile() && (e.name === target + ".java" || e.name === target + ".kt" || e.name === target + ".ts")) { found = true; return; }
										if (e.isDirectory()) walk(full, dep + 1);
									}
								};
								walk(srcBase, 0);
								return found;
							};
							const ctxNames = new Map<string, string>(); // norm(ctx) -> ctx
							const svcByCtx = new Map<string, string[]>();
							for (const sv of svcV) {
								const ctx = String(sv.context ?? ""); const nm = String(sv.name ?? "");
								if (!ctx || !nm) continue;
								ctxNames.set(norm(ctx), ctx);
								const arr = svcByCtx.get(ctx) ?? [];
								arr.push(nm); svcByCtx.set(ctx, arr);
							}
							let issues = 0;
							for (const [normCtx, ctx] of ctxNames) {
								const docFile = vModules.find((f) => norm(f.replace(/\.md$/, "")) === normCtx) ?? "";
								if (!docFile) {
									checks.push("  FAIL Module doc not found for context " + ctx + " (has " + svcByCtx.get(ctx)!.length + " catalog services)");
									issues++; continue;
								}
								const docText = fs.readFileSync(path.join(modulesDir, docFile), "utf-8");
								const catNames = svcByCtx.get(ctx)!;
								let miss = 0;
								for (const nm of catNames) if (!docText.includes(nm)) miss++;
								if (miss > 0) { checks.push("  FAIL " + docFile + " missing " + miss + " catalog service name(s): " + catNames.filter((n) => !docText.includes(n)).join(", ")); issues += miss; }
								if (!docText.includes("Service (canonical)")) {
									checks.push("  FAIL " + docFile + " has no Application Layer catalog block (header 'Service (canonical)') for " + catNames.length + " catalog services");
									issues++;
								}
								// Parse catalog table rows: | `Service` | use case | fr | Status | Delivered by |
								const block = docText.slice(docText.indexOf("### Application Layer") >= 0 ? docText.indexOf("### Application Layer") : 0);
								for (const line of block.split("\n")) {
									if (!line.trim().startsWith("|")) continue;
									const cells = line.split("|").map((c) => c.trim());
									// Locate the mapsToFr column (FR-00x[, FR-00y]*) — use-case
									// text may contain escaped pipes (\|) which naive split
									// fragments, so fixed indexes are unreliable.
									const frIdx = cells.findIndex((c, i) => i >= 2 && /^((FR|NFR)-\d[\d, ]*)(,\s*(FR|NFR)-\d[\d, ]*)*$/.test(c));
									if (cells.length < 6 || frIdx < 0) continue;
									const svcName = cells[1].replace(/^`|`$/g, "");
									if (!catNames.includes(svcName)) continue; // catalog row only
									const statusCell = (cells[frIdx + 1] ?? "") + " " + (cells[frIdx + 2] ?? "");
									const seamCell = cells[frIdx + 2] ?? "";
									if (/implemented/i.test(statusCell) && !/planned|seam/i.test(statusCell)) {
										const seamTokens = Array.from(seamCell.matchAll(/`([A-Za-z0-9_]+)`/g)).map((m) => m[1]);
										if (seamTokens.length === 0 && !/[A-Z][A-Za-z0-9_]+/.test(seamCell)) {
											checks.push("  FAIL " + docFile + ": Implemented catalog service " + svcName + " has no seam class named in the doc");
											issues++;
										} else {
											for (const tk of seamTokens.length > 0 ? seamTokens : seamCell.match(/[A-Z][A-Za-z0-9_]+/g) ?? []) {
												if (!typeExists(tk)) {
													checks.push("  FAIL " + docFile + ": Implemented service " + svcName + " names seam " + tk + " but no such type exists in src");
													issues++;
												}
											}
										}
									} else if (/planned|seam/i.test(statusCell)) {
										if (!/phase|roadmap|later|projection|pending|seam|intake|console|partial|deterministic/i.test(statusCell)) {
											checks.push("  FAIL " + docFile + ": catalog service " + svcName + " is Planned/Seam without a roadmap/phase pointer");
											issues++;
										}
									}
								}
							}
							checks.push(issues === 0
								? "  PASS Catalog blocks + seams verified for all " + svcV.length + " application services"
								: "  FAIL " + issues + " catalog/seam issue(s)");
							if (issues > 0) allPassed = false;
						} else {
							checks.push("  PASS No application services in session (use-cases pass not run)");
						}
					} catch { /* ignore */ }
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
					"  /domain --enrich <session-id>",
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
				"  /domain --enrich <session-id>",
				"    Enrich module stubs with full DDD architecture content",
				"",
				"  /domain --validate <session-id>",
				"    Validate exploration session structure"
			].join("\n");
		},
	});
}
