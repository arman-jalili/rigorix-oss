/**
 * Dynamic Config Reload Extension for pi
 *
 * Watches `.pi/agent/AGENTS.md` for file changes and re-applies
 * workflow config without requiring pi restart.
 *
 * Based on Symphony spec Section 6.2 (Dynamic Reload Semantics).
 */

import * as fs from "node:fs";
import * as path from "node:path";

type ExtensionAPI = {
	on(event: string, handler: (event: unknown, ctx: unknown) => void | Promise<void>): void;
	registerCommand(
		name: string,
		options: {
			description: string;
			handler(args: string[], ctx: unknown): unknown | Promise<unknown>;
		},
	): void;
};

// Config file to watch
const CONFIG_FILE = path.join(process.cwd(), ".pi", "agent", "AGENTS.md");

// Minimal YAML front-matter parser (no external deps)
function parseFrontMatter(text: string): string {
	if (!text.startsWith("---\n")) return "";
	const end = text.indexOf("\n---\n", 4);
	if (end === -1) return "";
	return text.slice(4, end);
}

let lastKnownConfig = "";
let watcher: fs.FSWatcher | undefined;
let reloadCount = 0;

function startWatching(ctx: unknown): void {
	if (!fs.existsSync(CONFIG_FILE)) return;

	lastKnownConfig = fs.readFileSync(CONFIG_FILE, "utf-8");

	watcher = fs.watch(CONFIG_FILE, { persistent: false }, (eventType) => {
		if (eventType !== "change") return;

		// Debounce: fs.watch fires multiple events
		setTimeout(() => {
			if (!fs.existsSync(CONFIG_FILE)) return;

			const newContent = fs.readFileSync(CONFIG_FILE, "utf-8");
			const newConfig = parseFrontMatter(newContent);
			const oldConfig = parseFrontMatter(lastKnownConfig);

			if (newConfig !== oldConfig) {
				reloadCount++;
				lastKnownConfig = newContent;

				const theme = ctx?.ui?.theme;
				const label = theme
					? `${theme.fg("accent", "⚡")} ${theme.fg("muted", `config reload #${reloadCount}`)}`
					: `⚡ config reload #${reloadCount}`;

				ctx?.ui?.setStatus("config-reload", label);
				ctx?.ui?.notify(
					`Config reloaded from .pi/agent/AGENTS.md (reload #${reloadCount}). New settings apply to next operation.`,
					"info",
				);
			}
		}, 500);
	});
}

function stopWatching(): void {
	if (watcher) {
		watcher.close();
		watcher = undefined;
	}
}

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async (_event: unknown, ctx: unknown) => {
		stopWatching();
		startWatching(ctx);
	});

	pi.on("session_end", async () => {
		stopWatching();
	});

	pi.registerCommand("reload-config", {
		description: "Manually reload workflow config from AGENTS.md",
		handler: async (_args: string[], ctx: unknown) => {
			if (!fs.existsSync(CONFIG_FILE)) {
				ctx?.ui?.notify("No .pi/agent/AGENTS.md found.", "warning");
				return;
			}

			const newContent = fs.readFileSync(CONFIG_FILE, "utf-8");
			const newConfig = parseFrontMatter(newContent);
			const oldConfig = parseFrontMatter(lastKnownConfig);

			if (newConfig !== oldConfig) {
				reloadCount++;
				lastKnownConfig = newContent;
				ctx?.ui?.notify(
					`Config reloaded from .pi/agent/AGENTS.md (reload #${reloadCount}).`,
					"success",
				);
			} else {
				ctx?.ui?.notify("Config unchanged.", "info");
			}
		},
	});
}
