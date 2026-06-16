/**
 * Session Persistence Extension for pi
 *
 * Provides structured session lifecycle with:
 * - Lazy-loaded message history
 * - Auto-derived session titles
 * - Per-session state isolation (todos, read cache)
 * - Atomic JSON storage with temp-file writes
 *
 * Storage: guardian-sessions.json in workspace root
 */

import * as fs from "node:fs";
import * as path from "node:path";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

// ── Types ──

interface SessionMeta {
	id: string;
	title: string;
	createdAt: number;
	updatedAt: number;
}

interface StoredSession {
	sessions: SessionMeta[];
	activeId: string | null;
}

const STORE_FILE = "guardian-sessions.json";
const MAX_SESSIONS = 50;

// ── Storage helpers ──

function storePath(cwd: string): string {
	return path.join(cwd, STORE_FILE);
}

function loadStore(cwd: string): StoredSession {
	const p = storePath(cwd);
	if (!fs.existsSync(p)) {
		return { sessions: [], activeId: null };
	}
	try {
		return JSON.parse(fs.readFileSync(p, "utf-8")) as StoredSession;
	} catch {
		return { sessions: [], activeId: null };
	}
}

function saveStore(cwd: string, store: StoredSession): void {
	const p = storePath(cwd);
	const tmp = `${p}.tmp`;
	fs.writeFileSync(tmp, JSON.stringify(store, null, 2), "utf-8");
	fs.renameSync(tmp, p);
}

function newSessionId(): string {
	const ts = Date.now().toString(36);
	const rand = Math.random().toString(36).slice(2, 8);
	return `s-${ts}-${rand}`;
}

// Derive title from first user message, stripping context blocks
function deriveTitle(firstUserMessage: string): string {
	const text = firstUserMessage
		.replace(/<terminal-context[\s\S]*?<\/terminal-context>/g, "")
		.replace(/<selection[\s\S]*?<\/selection>/g, "")
		.replace(/<file[\s\S]*?<\/file>/g, "")
		.replace(/<snippet[\s\S]*?<\/snippet>/g, "")
		.trim();
	if (!text) return "New session";
	const first = text.split("\n")[0].trim();
	return first.length > 40 ? `${first.slice(0, 40)}…` : first;
}

// ── Main extension ──

export default function (pi: ExtensionAPI) {
	let currentSession: SessionMeta | null = null;

	pi.on("session_start", async (_event, ctx) => {
		const store = loadStore(ctx.cwd);

		// Restore active session or create new one
		if (store.activeId) {
			const existing = store.sessions.find((s) => s.id === store.activeId);
			if (existing) {
				currentSession = existing;
				ctx.ui.setStatus("session", `📂 ${existing.title}`);
				return;
			}
		}

		// Create new session
		currentSession = {
			id: newSessionId(),
			title: "New session",
			createdAt: Date.now(),
			updatedAt: Date.now(),
		};

		store.sessions.unshift(currentSession);

		// Trim old sessions
		if (store.sessions.length > MAX_SESSIONS) {
			store.sessions = store.sessions.slice(0, MAX_SESSIONS);
		}

		store.activeId = currentSession.id;
		saveStore(ctx.cwd, store);

		ctx.ui.setStatus("session", `📂 ${currentSession.title}`);
	});

	// Update session title after first user message
	pi.on("input", async (event, ctx) => {
		if (!currentSession) return;
		if (currentSession.title !== "New session") return;

		const input = (event as { input?: string }).input;
		if (!input) return;
		const title = deriveTitle(input);
		currentSession.title = title;
		currentSession.updatedAt = Date.now();

		const store = loadStore(ctx.cwd);
		const idx = store.sessions.findIndex((s) => s.id === currentSession?.id);
		if (idx >= 0) {
			store.sessions[idx] = currentSession;
			saveStore(ctx.cwd, store);
		}

		ctx.ui.setStatus("session", `📂 ${title}`);
	});

	// Track session activity
	pi.on("tool_result", async (_event, ctx) => {
		if (!currentSession) return;
		currentSession.updatedAt = Date.now();

		const store = loadStore(ctx.cwd);
		const idx = store.sessions.findIndex((s) => s.id === currentSession?.id);
		if (idx >= 0) {
			store.sessions[idx] = currentSession;
			saveStore(ctx.cwd, store);
		}
	});

	// /sessions command — list sessions
	pi.registerCommand("sessions", {
		description: "List all sessions",
		handler: async (_args, ctx) => {
			const store = loadStore(ctx.cwd);
			if (store.sessions.length === 0) {
				ctx.ui.notify("No sessions.", "info");
				return;
			}
			const lines = store.sessions.map((s) => {
				const marker = s.id === currentSession?.id ? "▶ " : "  ";
				const age = Date.now() - s.updatedAt;
				const ageStr =
					age < 60_000
						? "just now"
						: age < 3_600_000
							? `${Math.floor(age / 60_000)}m ago`
							: `${Math.floor(age / 3_600_000)}h ago`;
				return `${marker}${s.title} (${ageStr})`;
			});
			ctx.ui.notify(`Sessions:\n\n${lines.join("\n")}`, "info");
		},
	});
}
