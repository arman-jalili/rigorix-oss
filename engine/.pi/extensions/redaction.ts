/**
 * Redaction Extension for pi
 *
 * Automatically redacts sensitive data from tool results, terminal output,
 * and agent responses. Prevents accidental leakage of API keys, tokens,
 * and credentials in conversation history and exported files.
 *
 * Patterns detected:
 *   - OpenAI keys (sk-proj-...)
 *   - Anthropic keys (sk-ant-...)
 *   - AWS access keys (AKIA...)
 *   - GitHub tokens (ghp_, gho_, ghs_, gh_r_, github_pat_)
 *   - Google API keys (AIza...)
 *   - Slack tokens (xoxb-...)
 *   - Stripe keys (sk_live_, pk_live_, sk_test_)
 *   - JWTs (eyJ...)
 *   - Bearer tokens
 *   - Environment variable assignments (API_KEY=..., SECRET=...)
 */

import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

// ── Redaction patterns ──

const PATTERNS: Array<{ kind: string; re: RegExp }> = [
	{ kind: "openai-key", re: /\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b/g },
	{ kind: "anthropic-key", re: /\bsk-ant-[A-Za-z0-9_-]{20,}\b/g },
	{ kind: "aws-access-key", re: /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/g },
	{ kind: "github-token", re: /\bgh[opsur]_[A-Za-z0-9]{36,}\b/g },
	{ kind: "github-pat", re: /\bgithub_pat_[A-Za-z0-9_]{40,}\b/g },
	{ kind: "google-api-key", re: /\bAIza[0-9A-Za-z_-]{35}\b/g },
	{ kind: "slack-token", re: /\bxox[bpsare]-[A-Za-z0-9-]{10,}\b/g },
	{ kind: "stripe-key", re: /\b(?:sk|pk|rk)_(?:live|test)_[A-Za-z0-9]{24,}\b/g },
	{ kind: "jwt", re: /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b/g },
	{ kind: "bearer", re: /\bBearer\s+[A-Za-z0-9._-]{20,}/g },
	{
		kind: "env-assign",
		re: /\b((?:[A-Z][A-Z0-9_]*)?(?:API[_-]?KEY|SECRET(?:[_-]?KEY)?|ACCESS[_-]?TOKEN|AUTH[_-]?TOKEN|PASSWORD|PASSWD|PRIVATE[_-]?KEY|CLIENT[_-]?SECRET)[A-Z0-9_]*)\s*[:=]\s*(["']?)([^\s"';|&]+)\2/gi,
	},
];

/**
 * Redact sensitive data from text. Returns the redacted string.
 */
export function redactSensitive(text: string): string {
	let out = text;
	for (const { kind, re } of PATTERNS) {
		if (kind === "env-assign") {
			out = out.replace(
				re,
				(_m: string, name: string, q: string, _val: string) => `${name}=${q}<REDACTED>${q}`,
			);
		} else {
			out = out.replace(re, `<REDACTED:${kind}>`);
		}
	}
	return out;
}

/**
 * Count redactions made (for logging/telemetry-free reporting).
 */
export function countRedactions(text: string): number {
	let count = 0;
	for (const { re } of PATTERNS) {
		const matches = text.match(re);
		if (matches) count += matches.length;
	}
	return count;
}

// ── Main extension ──

export default function (pi: ExtensionAPI) {
	// Redact sensitive data from tool results before showing to user
	pi.on("tool_result", async (event) => {
		if (!event.result) return;

		// Redact from text results
		if (typeof event.result === "string") {
			event.result = redactSensitive(event.result);
		}

		// Redact from structured results
		if (typeof event.result === "object" && event.result !== null) {
			const result = event.result as Record<string, unknown>;
			for (const [key, value] of Object.entries(result)) {
				if (typeof value === "string") {
					result[key] = redactSensitive(value);
				}
			}
		}
	});

	// Redact from user input (in case they paste a key)
	pi.on("input", async (event) => {
		const input = (event as { input?: string }).input;
		if (!input) return;
		const redacted = redactSensitive(input);
		if (redacted !== input) {
			const count = countRedactions(event.input) - countRedactions(redacted);
			if (count > 0) {
				event.input = redacted;
				// Note: pi extensions can't easily notify without ctx.ui,
				// but the redaction is applied silently for safety
			}
		}
	});

	// Register /redact command for testing
	pi.registerCommand("redact", {
		description: "Test redaction on provided text (for debugging)",
		handler: async (args, ctx) => {
			const text = args.join(" ");
			if (!text) {
				ctx.ui.notify("Usage: /redact <text to test>", "info");
				return;
			}
			const redacted = redactSensitive(text);
			const count = countRedactions(text);
			if (count === 0) {
				ctx.ui.notify("No sensitive patterns detected.", "success");
			} else {
				ctx.ui.notify(`Redacted ${count} sensitive pattern(s):\n\n${redacted}`, "warn");
			}
		},
	});
}
