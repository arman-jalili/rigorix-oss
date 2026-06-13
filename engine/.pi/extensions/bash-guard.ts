/**
 * Bash-Guard Extension for pi
 *
 * Interactively blocks destructive shell commands before execution.
 * Prompts the user with a risk assessment for flagged commands.
 *
 * In subagent sessions (headless), catastrophic operations are hard-blocked
 * without prompting.
 *
 * Zero dependencies — no shell-quote, no diff.
 */

import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { isToolCallEventType } from "@mariozechner/pi-coding-agent";
import type { SelectItem } from "@mariozechner/pi-tui";
import { Container, SelectList, Text } from "@mariozechner/pi-tui";

type Severity = "high" | "medium";

type Risk = {
	severity: Severity;
	reasons: string[];
};

// Simple shell command tokenizer — extracts command name and arguments.
// Handles: single/double quotes, escaped chars, pipes, redirects, &&/||/;
function tokenizeCommand(cmd: string): { parts: string[]; ops: string[] } {
	const parts: string[] = [];
	const ops: string[] = [];
	let current = "";
	let inSingleQuote = false;
	let inDoubleQuote = false;
	let escaped = false;

	for (let i = 0; i < cmd.length; i++) {
		const ch = cmd[i];

		if (escaped) {
			current += ch;
			escaped = false;
			continue;
		}

		if (ch === "\\") {
			escaped = true;
			continue;
		}

		if (ch === "'" && !inDoubleQuote) {
			inSingleQuote = !inSingleQuote;
			continue;
		}

		if (ch === '"' && !inSingleQuote) {
			inDoubleQuote = !inDoubleQuote;
			continue;
		}

		if (inSingleQuote || inDoubleQuote) {
			current += ch;
			continue;
		}

		// Operators: |, ||, &&, ;, >, >>, <
		if (ch === "|" || ch === ";" || ch === "<" || ch === ">") {
			if (current.trim()) parts.push(current.trim());
			current = "";

			// Check for ||, &&, >>
			const next = cmd[i + 1];
			if (
				(ch === "|" && next === "|") ||
				(ch === "&" && next === "&") ||
				(ch === ">" && next === ">")
			) {
				ops.push(ch + next);
				i++;
			} else {
				ops.push(ch);
			}
			continue;
		}

		if (ch === " " || ch === "\t") {
			if (current.trim()) parts.push(current.trim());
			current = "";
			continue;
		}

		current += ch;
	}

	if (current.trim()) parts.push(current.trim());
	return { parts, ops };
}

function analyzeCommand(command: string): Risk | null {
	const reasons: string[] = [];
	let severity: Severity = "medium";

	const { parts, ops } = tokenizeCommand(command);
	if (parts.length === 0) return null;

	const cmd = parts[0];
	const rest = parts.slice(1);

	const hasFlag = (flag: string): boolean =>
		rest.some((a) => a === flag || a.startsWith(`${flag}=`));
	const hasFlagPrefix = (prefix: string): boolean => rest.some((a) => a.startsWith(prefix));

	// Shell injection
	if (
		ops.includes("|") &&
		(rest.includes("sh") || rest.includes("bash") || rest.includes("zsh") || rest.includes("fish"))
	) {
		reasons.push("pipe to a shell (possible remote code execution)");
		severity = "high";
	}

	// sudo
	if (cmd === "sudo") {
		reasons.push("sudo (elevated privileges)");
		severity = "high";
	}

	// rm/rmdir/unlink
	if (cmd === "rm" || cmd === "rmdir" || cmd === "unlink") {
		severity = "high";
		reasons.push(`${cmd} (file deletion)`);
		if (
			hasFlag("-r") ||
			hasFlag("-R") ||
			hasFlagPrefix("-rf") ||
			hasFlagPrefix("-fr") ||
			hasFlagPrefix("-Rf") ||
			hasFlagPrefix("-fR")
		) {
			reasons.push("recursive delete (-r/-R)");
		}
		if (hasFlag("-f")) {
			reasons.push("forced delete (-f)");
		}
		if (rest.some((a) => a.includes("*") || a.includes("?"))) {
			reasons.push("glob pattern expansion (may delete many files)");
		}
	}

	// find -delete
	if (cmd === "find" && hasFlag("-delete")) {
		severity = "high";
		reasons.push("find -delete (bulk deletion)");
	}

	// Git operations
	if (cmd === "git") {
		const sub = rest[0];
		const subArgs = rest.slice(1);

		reasons.push(sub ? `git ${sub}` : "git");

		if (sub === "rm") {
			severity = "high";
			reasons.push("git rm (deletes files from working tree)");
		}
		if (sub === "clean" && (hasFlag("-f") || hasFlag("-d") || hasFlag("-x"))) {
			severity = "high";
			reasons.push("git clean (can delete untracked files)");
		}
		if (sub === "reset" && hasFlag("--hard")) {
			severity = "high";
			reasons.push("git reset --hard (discard changes)");
		}
		if (
			(sub === "checkout" || sub === "restore") &&
			(subArgs.includes(".") || hasFlag("--") || hasFlag("--source"))
		) {
			reasons.push("git checkout/restore (can overwrite working tree)");
		}
		if (sub === "push" && (hasFlag("--force") || hasFlag("--force-with-lease") || hasFlag("-f"))) {
			severity = "high";
			reasons.push("git push --force (rewrite remote history)");
		}
		if (sub === "reflog" && hasFlag("expire")) {
			severity = "high";
			reasons.push("git reflog expire (can remove recovery history)");
		}
		if (sub === "gc" && hasFlagPrefix("--prune")) {
			severity = "high";
			reasons.push("git gc --prune (can permanently delete objects)");
		}
	}

	// truncate
	if (cmd === "truncate") {
		reasons.push("truncate (in-place size change, can erase contents)");
	}

	// dd
	if (cmd === "dd" && hasFlagPrefix("of=")) {
		severity = "high";
		reasons.push("dd with output file/device (can overwrite data)");
	}

	// Disk / volume management
	const diskCmds = [
		"mkfs",
		"wipefs",
		"parted",
		"fdisk",
		"gdisk",
		"sgdisk",
		"lsblk",
		"cryptsetup",
		"zpool",
		"diskutil",
		"hdiutil",
		"gpt",
		"asr",
	];
	for (const dc of diskCmds) {
		if (cmd === dc || cmd.startsWith(`${dc}.`) || cmd.startsWith(`${dc}_`)) {
			severity = "high";
			reasons.push(`${cmd} (disk/partition management)`);
			break;
		}
	}

	// chmod/chown recursive
	if (cmd === "chmod" && (hasFlag("-R") || hasFlag("--recursive"))) {
		reasons.push("chmod -R (recursive permission changes)");
	}
	if (cmd === "chown" && (hasFlag("-R") || hasFlag("--recursive"))) {
		reasons.push("chown -R (recursive ownership changes)");
	}

	// mv/cp forcing
	if (cmd === "mv" && (hasFlag("-f") || hasFlag("--force"))) {
		reasons.push("mv --force (can overwrite files)");
	}
	if (cmd === "cp" && (hasFlag("-f") || hasFlag("--force"))) {
		reasons.push("cp --force (can overwrite files)");
	}

	// sed/perl in-place
	if (cmd === "sed" && rest.some((a) => a.startsWith("-i") || a === "--in-place")) {
		reasons.push("sed -i (in-place file modification)");
	}
	if (cmd === "perl" && (hasFlag("-pi") || (hasFlag("-p") && hasFlag("-i")))) {
		reasons.push("perl -pi (in-place file modification)");
	}

	// kill/shutdown/systemctl
	if (cmd === "kill" || cmd === "pkill" || cmd === "killall") {
		reasons.push(`${cmd} (process termination)`);
		if (hasFlag("-9")) {
			severity = "high";
			reasons.push("SIGKILL (-9)");
		}
	}
	if (cmd === "shutdown" || cmd === "reboot") {
		severity = "high";
		reasons.push(`${cmd} (system power operation)`);
	}
	if (cmd === "systemctl" && (hasFlag("stop") || hasFlag("disable"))) {
		reasons.push("systemctl stop/disable (service disruption)");
	}

	// Remote execution
	if ((cmd === "curl" || cmd === "wget") && ops.includes("|")) {
		severity = "high";
		reasons.push("curl/wget piped (possible remote code execution)");
	}

	// Infra destroys
	if (cmd === "kubectl" && rest[0] === "delete") {
		severity = "high";
		reasons.push("kubectl delete (resource deletion)");
	}
	if (cmd === "terraform" && rest[0] === "destroy") {
		severity = "high";
		reasons.push("terraform destroy (infrastructure teardown)");
	}
	if (cmd === "aws" && rest[0] === "s3" && rest[1] === "rm" && hasFlag("--recursive")) {
		severity = "high";
		reasons.push("aws s3 rm --recursive (bulk deletion)");
	}

	if (reasons.length === 0) return null;
	return { severity, reasons: [...new Set(reasons)] };
}

// biome-ignore lint/suspicious/noExplicitAny: pi ExtensionContext has no published types
async function promptRunOrAbort(ctx: any, command: string, risk: Risk): Promise<"run" | "abort"> {
	if (!ctx.hasUI) return "abort";

	const reasonsText = risk.reasons.map((r) => `• ${r}`).join("\n");
	const body = `Risk: ${risk.severity.toUpperCase()}\n\n${reasonsText}\n\nCommand:\n${command}`;

	const items: SelectItem[] = [
		{ value: "run", label: "Run", description: "Execute the command" },
		{ value: "abort", label: "Abort", description: "Block this command" },
	];

	const choice = await ctx.ui.custom<"run" | "abort">(
		(tui, theme, _kb, done) => {
			const container = new Container();
			container.addChild(
				new Text(theme.fg("warning", theme.bold("⚠  Potentially destructive bash command")), 1, 0),
			);
			container.addChild(new Text(body, 1, 0));

			const list = new SelectList(items, items.length, {
				selectedPrefix: (t) => theme.fg("accent", t),
				selectedText: (t) => theme.fg("accent", t),
				description: (t) => theme.fg("muted", t),
				scrollInfo: (t) => theme.fg("dim", t),
				noMatch: (t) => theme.fg("warning", t),
			});

			list.onSelect = (item) => done(item.value as "run" | "abort");
			list.onCancel = () => done("abort");
			container.addChild(list);

			return {
				render: (w) => container.render(w),
				invalidate: () => container.invalidate(),
				handleInput: (data) => {
					list.handleInput(data);
					tui.requestRender();
				},
			};
		},
		{ overlay: true },
	);

	return choice ?? "abort";
}

// PI_SUBAGENT_DEPTH is 0 (or unset) in the main session and >= 1 in spawned subagent processes.
const subagentDepth = Number(process.env.PI_SUBAGENT_DEPTH ?? "0");
const isSubagent = Number.isFinite(subagentDepth) && subagentDepth >= 1;

// ── Path safety guards (Terax-inspired) ──
// Pre-execution blocks for reads/writes of sensitive files and system directories.

const SECRET_BASENAME_PATTERNS: RegExp[] = [
	/^\.env(\..+)?$/i, // .env, .env.local, .env.production, etc.
	/^.*\.pem$/i,
	/^.*\.key$/i, // private keys
	/^.*\.p12$/i,
	/^.*\.pfx$/i,
	/^id_(rsa|dsa|ecdsa|ed25519)(\.pub)?$/i,
	/^known_hosts$/i,
	/^authorized_keys$/i,
	/^htpasswd$/i,
	/^\.netrc$/i,
	/^credentials$/i,
	/^\.pgpass$/i,
	/^\.npmrc$/i,
	/^\.pypirc$/i,
	/^secrets?\.(json|ya?ml|toml)$/i,
];

const SECRET_PATH_SEGMENTS = [
	"/.ssh/",
	"/.gnupg/",
	"/.aws/",
	"/.azure/",
	"/.kube/",
	"/.docker/",
	"/.config/gh/",
	"/.config/git/",
	"/.git/",
];

const FORBIDDEN_WRITE_PREFIXES = [
	"/etc/",
	"/var/db/",
	"/System/",
	"/Library/Keychains/",
	"/private/etc/",
	"/private/var/db/",
];

function basename(p: string): string {
	const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
	return i >= 0 ? p.slice(i + 1) : p;
}

function normalizePath(p: string): string {
	return p.replace(/\\/g, "/");
}

/** Check if a file path is safe to read. Refuses obvious secret files. */
export function checkReadable(filePath: string): { ok: true } | { ok: false; reason: string } {
	const norm = normalizePath(filePath);
	const base = basename(norm);

	for (const re of SECRET_BASENAME_PATTERNS) {
		if (re.test(base)) {
			return { ok: false, reason: `Refused: "${base}" matches a sensitive-file pattern.` };
		}
	}

	for (const seg of SECRET_PATH_SEGMENTS) {
		if (norm.includes(seg)) {
			return {
				ok: false,
				reason: `Refused: path is inside a protected directory (${seg.trim()}).`,
			};
		}
	}

	return { ok: true };
}

/** Check if a file path is safe to write. Inherits read restrictions + system dir blocks. */
export function checkWritable(filePath: string): { ok: true } | { ok: false; reason: string } {
	const r = checkReadable(filePath);
	if (!r.ok) return r;

	const norm = normalizePath(filePath);
	for (const prefix of FORBIDDEN_WRITE_PREFIXES) {
		if (norm.startsWith(prefix)) {
			return { ok: false, reason: `Refused: writes under "${prefix}" are not allowed.` };
		}
	}
	return { ok: true };
}

/** Lightweight heuristic for blocking obviously destructive shell commands. */
export function checkShellCommand(cmd: string): { ok: true } | { ok: false; reason: string } {
	const c = cmd.trim();
	if (
		/\brm\s+(-[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*|-[a-zA-Z]*f[a-zA-Z]*r[a-zA-Z]*|--recursive\s+--force|--force\s+--recursive)\s+(['"]?\/?['"]?\s*($|;|&|\|))/.test(
			c,
		)
	) {
		return {
			ok: false,
			reason: "Refused: command attempts to recursively delete the filesystem root.",
		};
	}
	if (/--no-preserve-root/.test(c)) {
		return { ok: false, reason: "Refused: --no-preserve-root is not allowed." };
	}
	if (/\bdd\b[^|]*\bof=\/dev\/(disk|sd|nvme|hd)/i.test(c)) {
		return { ok: false, reason: "Refused: dd to a block device is not allowed." };
	}
	if (/\b(mkfs(\.[a-z0-9]+)?|fdisk|parted)\b/.test(c) || /\bdiskutil\s+erase/i.test(c)) {
		return { ok: false, reason: "Refused: disk-formatting commands are not allowed." };
	}
	return { ok: true };
}

// ── Hard-block patterns for subagent (headless) mode ──

const HEADLESS_BLOCKED: Array<{ pattern: RegExp; reason: string }> = [
	{
		pattern:
			/\brm\s+(-[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*|-[a-zA-Z]*f[a-zA-Z]*r[a-zA-Z]*)\s+(['"]?\/?['"]?\s*($|;|&|\|))/,
		reason: "recursive filesystem deletion (rm -rf /)",
	},
	{ pattern: /--no-preserve-root/, reason: "override root protection (--no-preserve-root)" },
	{
		pattern: /(?<!\bgit\s+)\brm\b[^#\n]*\s-(?:[a-zA-Z]*[rR]|-\brecursive\b)/,
		reason: "recursive delete (rm -r / -rf)",
	},
	{ pattern: /\bsudo\b/, reason: "elevated privileges (sudo)" },
	{
		pattern: /\b(curl|wget)\b[^#\n]*\|\s*(ba?sh|zsh|fish|dash|sh)\b/,
		reason: "pipe to shell (remote code execution)",
	},
	{ pattern: /\bmkfs/, reason: "filesystem formatting (mkfs)" },
	{ pattern: /\bwipefs\b/, reason: "disk signature wipe" },
	{
		pattern: /\bdiskutil\s+(erase|zeroDisk|secureErase|reformat)/i,
		reason: "destructive disk operation (diskutil)",
	},
	{ pattern: /\bdd\b[^#\n]*\bof=\/dev\//, reason: "raw disk write (dd of=/dev/...)" },
	{ pattern: /\b(parted|fdisk|gdisk|sgdisk)\b/, reason: "partition table management" },
	{ pattern: /\b(shutdown|reboot|halt|poweroff)\b/, reason: "system power operation" },
	{ pattern: /\bterraform\s+destroy\b/, reason: "infrastructure teardown (terraform destroy)" },
	{ pattern: /\bkubectl\s+delete\b/, reason: "Kubernetes resource deletion" },
	{
		pattern: /\baws\s+s3\s+rm\b[^#\n]*--recursive/,
		reason: "bulk S3 deletion (aws s3 rm --recursive)",
	},
	{ pattern: /\bgit\s+push\b/, reason: "git push (main-session operation)" },
	{ pattern: /\bgit\s+commit\b/, reason: "git commit (main-session operation)" },
	{
		pattern: /\bgit\s+reset\b[^#\n]*--hard\b/,
		reason: "discard all uncommitted changes (git reset --hard)",
	},
	{ pattern: /\bgit\s+clean\b[^#\n]*-[a-zA-Z]*f/, reason: "delete untracked files (git clean -f)" },
];

export default function (pi: ExtensionAPI) {
	if (isSubagent) {
		pi.on("tool_call", async (event) => {
			if (!isToolCallEventType("bash", event)) return;
			const command = event.input.command;
			for (const { pattern, reason } of HEADLESS_BLOCKED) {
				if (pattern.test(command)) {
					return {
						block: true,
						reason: `Blocked by bash-guard: ${reason}. This is a non-interactive subagent session — catastrophic operations are not permitted. Propose a safer alternative or ask the parent agent to confirm with the user.`,
					};
				}
			}
		});
		return;
	}

	pi.registerFlag("bash-guard-auto-allow", {
		description:
			"If set, bash-guard will not block when no UI is available (non-interactive modes).",
		type: "boolean",
		default: false,
	});

	const recentlyAborted = new Map<string, number>();
	const ABORT_REMEMBER_MS = 60_000;

	pi.on("tool_call", async (event, ctx) => {
		if (!isToolCallEventType("bash", event)) return;

		const command = event.input.command;
		const risk = analyzeCommand(command);
		if (!risk) return;

		const now = Date.now();
		const lastAbort = recentlyAborted.get(command);
		if (lastAbort && now - lastAbort < ABORT_REMEMBER_MS) {
			return {
				block: true,
				reason:
					"Blocked by bash-guard: command was already aborted recently. Ask the user for a safer alternative; do not retry the same command.",
			};
		}

		if (!ctx.hasUI && pi.getFlag("--bash-guard-auto-allow")) {
			return;
		}

		const choice = await promptRunOrAbort(ctx, command, risk);
		if (choice === "run") return;

		recentlyAborted.set(command, now);
		return {
			block: true,
			reason:
				"Blocked by user via bash-guard (potentially destructive command). Ask the user for confirmation or propose a non-destructive alternative.",
		};
	});
}
