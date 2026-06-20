// biome-ignore lint/suspicious/noExplicitAny: pi ExtensionContext has no published types
// biome-ignore lint/style/noNonNullAssertion: pi UI callbacks use non-null assertions
/**
 * Ask User Question Extension for pi
 *
 * biome-ignore lint/suspicious/noExplicitAny: pi ExtensionContext has no published types
 * biome-ignore lint/style/noNonNullAssertion: pi UI callbacks use non-null assertions
 *
 * Registers a structured question tool that supports free-text, single-select,
 * and multi-select with an "Other" escape hatch.
 *
 * Uses a mutex to serialize concurrent UI interactions (pi can only show one
 * custom prompt at a time).
 */

import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import {
	Editor,
	type EditorTheme,
	Key,
	Text,
	matchesKey,
	truncateToWidth,
	wrapTextWithAnsi,
} from "@mariozechner/pi-tui";
import { Type } from "typebox";

interface AskOption {
	label: string;
	value: string;
	description?: string;
}

interface DisplayOption extends AskOption {
	id: string;
	index?: number;
	isOther?: boolean;
	isSubmit?: boolean;
}

interface TextAnswer {
	type: "text";
	label: string;
	value: string;
}

interface OptionAnswer {
	type: "option";
	label: string;
	value: string;
	index: number;
}

interface OtherAnswer {
	type: "other";
	label: string;
	value: string;
}

type AskAnswer = TextAnswer | OptionAnswer | OtherAnswer;

function normalizeOptions(
	options: Array<{ label: string; value?: string; description?: string }> | undefined,
): AskOption[] {
	return (options || [])
		.map((o) => ({
			label: o.label.trim(),
			value: o.value?.trim() || o.label.trim(),
			description: o.description?.trim() || undefined,
		}))
		.filter((o) => o.label.length > 0);
}

function getOtherLabel(options: AskOption[]): string {
	return options.some((o) => o.label.toLowerCase() === "other") ? "Other (custom)" : "Other";
}

// biome-ignore lint/suspicious/noExplicitAny: pi ExtensionContext has no published types
function createEditorTheme(theme: any): EditorTheme {
	return {
		borderColor: (s) => theme.fg("accent", s),
		selectList: {
			selectedPrefix: (t) => theme.fg("accent", t),
			selectedText: (t) => theme.fg("accent", t),
			description: (t) => theme.fg("muted", t),
			scrollInfo: (t) => theme.fg("dim", t),
			noMatch: (t) => theme.fg("warning", t),
		},
	};
}

function addWrapped(lines: string[], text: string, width: number, indent = ""): void {
	const contentWidth = Math.max(1, width - indent.length);
	for (const line of wrapTextWithAnsi(text, contentWidth)) {
		lines.push(truncateToWidth(`${indent}${line}`, width));
	}
}

function formatAnswerForModel(answer: AskAnswer): string {
	switch (answer.type) {
		case "text":
			return answer.label;
		case "other":
			return `Other: ${answer.label}`;
		case "option":
			return `${answer.index}. ${answer.label}`;
	}
}

function answerSortRank(answer: AskAnswer): number {
	switch (answer.type) {
		case "option":
			return answer.index;
		case "other":
			return Number.MAX_SAFE_INTEGER - 1;
		case "text":
			return Number.MAX_SAFE_INTEGER;
	}
}

function sortAnswers(answers: AskAnswer[]): AskAnswer[] {
	return [...answers].sort((a, b) => answerSortRank(a) - answerSortRank(b));
}

function cancelledResult(question: string, context?: string) {
	const message = "User cancelled the question";
	return {
		content: [{ type: "text" as const, text: message }],
		details: { status: "cancelled", question, context, message },
	};
}

function unavailableResult(question: string, context?: string) {
	const message = "ask_user_question requires interactive mode UI";
	return {
		content: [{ type: "text" as const, text: message }],
		details: { status: "unavailable", question, context, message },
	};
}

function buildResult(
	question: string,
	context: string | undefined,
	answers: AskAnswer[],
	multiSelect: boolean,
) {
	let text: string;
	if (!multiSelect) {
		const answer = answers[0];
		text =
			answer.label.trim().length > 0
				? `User answered: ${answer.label}`
				: "User submitted an empty response";
	} else {
		text = `User selected:\n${answers.map((a) => `- ${formatAnswerForModel(a)}`).join("\n")}`;
	}
	return {
		content: [{ type: "text" as const, text }],
		details: {
			status: "answered",
			question,
			context,
			mode: multiSelect ? "multi-select" : "single-select",
			answers,
		},
	};
}

async function askSingleChoice(
	// biome-ignore lint/suspicious/noExplicitAny: pi ExtensionContext has no published types
	ctx: any,
	question: string,
	context: string | undefined,
	options: AskOption[],
): Promise<AskAnswer | null> {
	const otherLabel = getOtherLabel(options);
	const allOptions: DisplayOption[] = [
		...options.map((o, i) => ({ ...o, id: `option:${i}`, index: i + 1 })),
		{ id: "other", label: otherLabel, value: "__other__", isOther: true },
	];

	return ctx.ui.custom<AskAnswer | null>(
		(tui: any, theme: any, _kb: any, done: (r: AskAnswer | null) => void) => {
			let optionIndex = 0;
			let editMode = false;
			let cachedLines: string[] | undefined;
			const editor = new Editor(tui, createEditorTheme(theme));

			editor.onSubmit = (value: string) => {
				const trimmed = value.trim();
				if (!trimmed) return;
				done({ type: "other", label: trimmed, value: trimmed });
			};

			function refresh() {
				cachedLines = undefined;
				tui.requestRender();
			}

			function handleInput(data: string) {
				if (editMode) {
					if (matchesKey(data, Key.escape)) {
						editMode = false;
						editor.setText("");
						refresh();
						return;
					}
					editor.handleInput(data);
					refresh();
					return;
				}
				if (matchesKey(data, Key.up)) {
					optionIndex = Math.max(0, optionIndex - 1);
					refresh();
					return;
				}
				if (matchesKey(data, Key.down)) {
					optionIndex = Math.min(allOptions.length - 1, optionIndex + 1);
					refresh();
					return;
				}
				if (matchesKey(data, Key.enter)) {
					const selected = allOptions[optionIndex];
					if (selected.isOther) {
						editMode = true;
						editor.setText("");
						refresh();
						return;
					}
					done({
						type: "option",
						label: selected.label,
						value: selected.value,
						index: selected.index ?? 0,
					});
					return;
				}
				if (matchesKey(data, Key.escape)) {
					done(null);
				}
			}

			function render(width: number): string[] {
				if (cachedLines) return cachedLines;
				const lines: string[] = [];
				const add = (s: string) => lines.push(truncateToWidth(s, width));

				add(theme.fg("accent", "─".repeat(width)));
				addWrapped(lines, theme.fg("text", ` ${question}`), width);
				if (context) {
					lines.push("");
					addWrapped(lines, theme.fg("muted", ` ${context}`), width);
				}
				lines.push("");

				for (let i = 0; i < allOptions.length; i++) {
					const o = allOptions[i];
					const sel = i === optionIndex;
					const prefix = sel ? theme.fg("accent", "> ") : "  ";
					const label = o.isOther ? o.label : `${o.index}. ${o.label}`;
					add(`${prefix}${sel ? theme.fg("accent", label) : theme.fg("text", label)}`);
					if (o.description) addWrapped(lines, theme.fg("muted", o.description), width, "     ");
				}

				if (editMode) {
					lines.push("");
					add(theme.fg("muted", " Write your custom answer:"));
					for (const line of editor.render(Math.max(1, width - 2))) add(` ${line}`);
					lines.push("");
					add(theme.fg("dim", " Enter to submit • Esc to go back"));
				} else {
					lines.push("");
					add(theme.fg("dim", " ↑↓ navigate • Enter select • Esc cancel"));
				}

				add(theme.fg("accent", "─".repeat(width)));
				cachedLines = lines;
				return lines;
			}

			return {
				render,
				invalidate: () => {
					cachedLines = undefined;
				},
				handleInput,
			};
		},
	);
}

async function askMultiChoice(
	// biome-ignore lint/suspicious/noExplicitAny: pi ExtensionContext has no published types
	ctx: any,
	question: string,
	context: string | undefined,
	options: AskOption[],
): Promise<AskAnswer[] | null> {
	const otherLabel = getOtherLabel(options);
	const allItems: DisplayOption[] = [
		...options.map((o, i) => ({ ...o, id: `option:${i}`, index: i + 1 })),
		{ id: "other", label: otherLabel, value: "__other__", isOther: true },
		{ id: "submit", label: "Submit", value: "__submit__", isSubmit: true },
	];

	return ctx.ui.custom<AskAnswer[] | null>(
		(tui: any, theme: any, _kb: any, done: (r: AskAnswer[] | null) => void) => {
			let optionIndex = 0;
			let editMode = false;
			let cachedLines: string[] | undefined;
			const selected = new Map<string, AskAnswer>();
			const editor = new Editor(tui, createEditorTheme(theme));

			editor.onSubmit = (value: string) => {
				const trimmed = value.trim();
				if (!trimmed) return;
				selected.set("other", { type: "other", label: trimmed, value: trimmed });
				editMode = false;
				refresh();
			};

			function refresh() {
				cachedLines = undefined;
				tui.requestRender();
			}
			function toggleOption(item: DisplayOption) {
				if (selected.has(item.id)) selected.delete(item.id);
				else
					selected.set(item.id, {
						type: "option",
						label: item.label,
						value: item.value,
						index: item.index ?? 0,
					});
				refresh();
			}

			function handleInput(data: string) {
				if (editMode) {
					if (matchesKey(data, Key.escape)) {
						editMode = false;
						editor.setText(selected.get("other")?.label || "");
						refresh();
						return;
					}
					editor.handleInput(data);
					refresh();
					return;
				}
				if (matchesKey(data, Key.up)) {
					optionIndex = Math.max(0, optionIndex - 1);
					refresh();
					return;
				}
				if (matchesKey(data, Key.down)) {
					optionIndex = Math.min(allItems.length - 1, optionIndex + 1);
					refresh();
					return;
				}
				const current = allItems[optionIndex];
				if (matchesKey(data, Key.space) || matchesKey(data, Key.enter)) {
					if (current.isSubmit) {
						if (selected.size > 0) done(sortAnswers(Array.from(selected.values())));
						return;
					}
					if (current.isOther) {
						if (selected.has("other")) {
							selected.delete("other");
							refresh();
						} else {
							editMode = true;
							editor.setText("");
							refresh();
						}
						return;
					}
					toggleOption(current);
					return;
				}
				if (matchesKey(data, Key.escape)) {
					done(null);
				}
			}

			function render(width: number): string[] {
				if (cachedLines) return cachedLines;
				const lines: string[] = [];
				const add = (s: string) => lines.push(truncateToWidth(s, width));

				add(theme.fg("accent", "─".repeat(width)));
				addWrapped(lines, theme.fg("text", ` ${question}`), width);
				if (context) {
					lines.push("");
					addWrapped(lines, theme.fg("muted", ` ${context}`), width);
				}
				lines.push("");

				for (let i = 0; i < allItems.length; i++) {
					const item = allItems[i];
					const focused = i === optionIndex;
					const prefix = focused ? theme.fg("accent", "> ") : "  ";

					if (item.isSubmit) {
						const label =
							selected.size > 0 ? `✓ ${item.label} (${selected.size} selected)` : `○ ${item.label}`;
						const styled = focused
							? theme.fg("accent", label)
							: theme.fg(selected.size > 0 ? "success" : "dim", label);
						add(`${prefix}${styled}`);
						continue;
					}
					if (item.isOther) {
						const other = selected.get("other");
						const marker = other ? "[x]" : "[ ]";
						const suffix = other ? ` — ${other.label}` : "";
						const styled = focused
							? theme.fg("accent", `${marker} ${item.label}${suffix}`)
							: theme.fg(other ? "success" : "text", `${marker} ${item.label}${suffix}`);
						add(`${prefix}${styled}`);
						continue;
					}
					const checked = selected.has(item.id);
					const marker = checked ? "[x]" : "[ ]";
					const label = `${marker} ${item.index}. ${item.label}`;
					const styled = focused
						? theme.fg("accent", label)
						: theme.fg(checked ? "success" : "text", label);
					add(`${prefix}${styled}`);
					if (item.description)
						addWrapped(lines, theme.fg("muted", item.description), width, "     ");
				}

				if (editMode) {
					lines.push("");
					add(theme.fg("muted", " Write your custom answer:"));
					for (const line of editor.render(Math.max(1, width - 2))) add(` ${line}`);
					lines.push("");
					add(theme.fg("dim", " Enter to save • Esc to go back"));
				} else {
					lines.push("");
					if (selected.size === 0)
						add(theme.fg("warning", " Select at least one answer before submitting."));
					add(theme.fg("dim", " ↑↓ navigate • Space toggle • Enter edit/submit • Esc cancel"));
				}

				add(theme.fg("accent", "─".repeat(width)));
				cachedLines = lines;
				return lines;
			}

			return {
				render,
				invalidate: () => {
					cachedLines = undefined;
				},
				handleInput,
			};
		},
	);
}

// Mutex to serialize concurrent UI interactions.
let uiLock: Promise<void> = Promise.resolve();

function withUILock<T>(fn: () => Promise<T>): Promise<T> {
	const prev = uiLock;
	let release: () => void;
	uiLock = new Promise<void>((r) => {
		release = r;
	});
	return prev.then(fn).finally(() => release?.());
}

export default function (pi: ExtensionAPI) {
	pi.registerTool({
		name: "ask_user_question",
		label: "Ask User Question",
		description:
			"Ask the user a single question and pause execution until they answer. Use when requirements are ambiguous, user preferences are needed, or a decision would materially affect implementation. Ask exactly one question per tool call.",
		promptSnippet: "Use this tool to ask exactly one clarifying question before continuing.",
		promptGuidelines: [
			"Ask exactly one question per tool call.",
			"Use multiple separate tool calls for multiple questions.",
			'Users will always be able to select "Other" to provide custom text input.',
			"Use multiSelect only when you need multiple answers to the same question.",
			'If you recommend a specific option, make it first and add "(Recommended)" to the label.',
			"Prefer this tool over guessing when requirements are unclear.",
		],
		parameters: Type.Object({
			question: Type.String({
				description: "The single question to ask. Ask exactly one question per tool call.",
			}),
			options: Type.Optional(
				Type.Array(
					Type.Object({
						label: Type.String({
							description:
								'Display label. Place recommended option first with "(Recommended)" appended.',
						}),
						value: Type.Optional(
							Type.String({ description: "Machine-readable value. Defaults to label." }),
						),
						description: Type.Optional(
							Type.String({ description: "Extra detail shown below the option." }),
						),
					}),
					{ description: "Multiple-choice options. Omit for free-form text." },
				),
			),
			multiSelect: Type.Optional(Type.Boolean({ description: "Allow multiple answers." })),
		}),

		async execute(_toolCallId, params, signal, _onUpdate, ctx) {
			const options = normalizeOptions(params.options);
			const context = params.details?.trim() || undefined;

			if (signal?.aborted) return cancelledResult(params.question, context);
			if (!ctx.hasUI) return unavailableResult(params.question, context);

			return withUILock(async () => {
				if (options.length === 0) {
					const answer = await ctx.ui.editor(
						context ? `${params.question}\n\n${context}` : params.question,
					);
					if (answer === undefined) return cancelledResult(params.question, context);
					return buildResult(
						params.question,
						context,
						[{ type: "text", label: answer.trim(), value: answer.trim() }],
						false,
					);
				}

				const multiSelect = params.multiSelect === true;
				if (!multiSelect) {
					const answer = await askSingleChoice(ctx, params.question, context, options);
					if (!answer) return cancelledResult(params.question, context);
					return buildResult(params.question, context, [answer], false);
				}

				const answers = await askMultiChoice(ctx, params.question, context, options);
				if (!answers) return cancelledResult(params.question, context);
				return buildResult(params.question, context, answers, true);
			});
		},

		renderCall(args, theme) {
			const opts = normalizeOptions(args.options);
			let text =
				theme.fg("toolTitle", theme.bold("ask_user_question ")) + theme.fg("muted", args.question);
			if (args.multiSelect) text += theme.fg("dim", " [multi-select]");
			if (opts.length > 0) {
				const labels = [...opts.map((o) => o.label), getOtherLabel(opts)].join(", ");
				text += `\n${theme.fg("dim", `  Options: ${labels}`)}`;
			}
			return new Text(text, 0, 0);
		},

		renderResult(result, _options, theme) {
			const details = result.details as
				| { status?: string; message?: string; answers?: AskAnswer[] }
				| undefined;
			if (!details) {
				const first = result.content[0];
				return new Text(first?.type === "text" ? first.text : "", 0, 0);
			}
			if (details.status === "cancelled")
				return new Text(theme.fg("warning", details.message || "Cancelled"), 0, 0);
			if (details.status === "unavailable")
				return new Text(theme.fg("warning", details.message || "Unavailable"), 0, 0);

			const lines = (details.answers || []).map((answer) => {
				const fmt = formatAnswerForModel(answer);
				return `${theme.fg("success", "✓ ")}${theme.fg("accent", fmt)}`;
			});
			return new Text(lines.join("\n"), 0, 0);
		},
	});
}
