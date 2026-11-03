/**
 * Architect Extension — Full Architecture-to-Implementation Pipeline
 *
 * OVERVIEW
 * ========
 * /architect is the planning engine that reads architecture module docs from
 * .pi/architecture/modules/ and produces epics + issues for implementation.
 * It is the bridge between domain exploration (/domain --explore) and
 * implementation (/implement-series).
 *
 * PREREQUISITES
 * ============
 * - .pi/architecture/modules/*.md must exist (from /domain --architect-scaffold
 *   or manually created)
 * - Git repository initialized (handled automatically if missing)
 * - gh or glab CLI authenticated for remote issue creation (optional)
 *
 * COMMANDS
 * =======
 * /architect --epic "Name" [--tracking-issue N]
 *   Starts a new epic: discovers architecture modules, finds the next
 *   planned slice, generates issues, creates pipeline state, and sends
 *   implementation instructions to the agent via pi.sendMessage() with
 *   triggerTurn=true.
 *
 * /architect status
 *   Shows current epic state: module, component, pipeline progress,
 *   issues, and validators.
 *
 * /architect next-epic
 *   Shows which module and components should be implemented next,
 *   based on component status (planned vs implemented).
 *
 * /architect abort
 *   Cancels the current epic, cleans pipeline state.
 *
 * TOOLS (agent-callable)
 * =====================
 * architect_status   — Returns current epic state and progress.
 * architect_discover — Discovers modules and finds next logical slice.
 *
 * WORKFLOW
 * =======
 * 1. /domain --explore "intent"          (discovery)
 * 2. /domain --architect-scaffold <id>   (generates modules)
 * 3. /architect --epic "Name"            (planning + issue gen)
 * 4. Agent implements issues via pipeline
 */

// ── Types ──

export type ExtensionContext = {
	cwd: string;
	ui: {
		notify(message: string, level?: string): void;
		setStatus(key: string, message: string | null): void;
		confirm(title: string, message: string): Promise<boolean>;
		theme?: unknown;
	};
	tools: { execute(name: string, params: Record<string, unknown>): Promise<unknown> };
};

export type ExtensionAPI = {
	on(
		event: string,
		handler: (event: unknown, ctx: ExtensionContext) => unknown | Promise<unknown>,
	): void;
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
	registerCommand(
		name: string,
		options: {
			description: string;
			handler(args: string, ctx: ExtensionContext): unknown | Promise<unknown>;
		},
	): void;
	sendMessage<T = unknown>(
		message: { customType?: string; content: string; display?: boolean; details?: Record<string, unknown> },
		options?: { deliverAs?: "steer" | "followUp" | "nextTurn"; triggerTurn?: boolean },
	): void;
	sendUserMessage(
		content: string,
		options?: { deliverAs?: "steer" | "followUp" },
	): void;
};

export type ModuleComponent = {
	name: string;
	status: "planned" | "in-progress" | "implemented" | "deprecated";
	description: string;
	dependencies: string[];
	/** ADR-008 modeling depth for this component's module (from exploration) */
	modelingDepth?: "tactical" | "service" | "adapter";
	/** True when this component is an aggregate root — slice selection keeps it atomic */
	aggregateRoot?: boolean;
	/** Aggregate invariants (ADR-008) — become frozen contract + TDD tests */
	invariants?: string[];
	/** Domain events this aggregate publishes/subscribes to (ADR-008) */
	events?: { publishes: string[]; subscribes: string[] };
};

export type ArchitectureSlice = {
	module: string;
	components: ModuleComponent[];
	nextLogicalSlice: ModuleComponent[];
};


/** A module row inside a roadmap phase (implementation-roadmap.md). */
export interface RoadmapPhaseModule {
	name: string;
	deliverables: string;
	doc: string;
}

/** A single delivery phase parsed from implementation-roadmap.md. */
export interface RoadmapPhase {
	index: number;
	title: string;
	goal: string;
	days: string;
	modules: RoadmapPhaseModule[];
	completedModules: string[];
	dependencies: string[];
	migrations: string[];
	criteria: string[];
	status: string; // "pending" | "in_progress" | "done"
}

/** Persisted roadmap execution state (.pi/.guardian-roadmap-state.json). */
export interface RoadmapState {
	phases: RoadmapPhase[];
	currentPhaseIndex: number;
	startedAt: string;
	updatedAt: string;
}

export type EpicState = {
	name: string;
	trackingIssueId: string | null;
	epicId: string | null;
	status: "planning" | "validating" | "publishing" | "executing" | "done" | "aborted";
	slices: ArchitectureSlice[];
	issues: { id: string; title: string; status: string; remoteIssueId?: string | null }[];
	currentIssueIndex: number;
	createdAt: string;
	tdd: boolean;
	tddTestFiles: string[];
};

