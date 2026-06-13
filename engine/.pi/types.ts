/**
 * Shared types for pi extensions.
 *
 * The pi coding agent (`@mariozechner/pi-coding-agent`) does not currently
 * publish a formal TypeScript type definition for its extension context.
 * These types mirror the runtime shape so template extensions can type-check
 * without `any`.
 */

export interface PiExtensionContext {
	cwd: string;
	hasUI: boolean;
	ui: PiUI;
	session: {
		tools: Record<string, { execute: (...args: unknown[]) => unknown }>;
	};
}

export interface PiUI {
	notify(message: string, type: "info" | "success" | "warn" | "error" | "warning"): void;
	confirm(title: string, message: string): Promise<boolean>;
	input(title: string, placeholder?: string): Promise<string | null>;
	select(title: string, options: string[]): Promise<string | null>;
	setStatus(key: string, message: string | null): void;
	setWidget(key: string, lines: string[]): void;
	custom<T>(
		render: (tui: PiTUI, theme: PiTheme, kb: unknown, done: (value: T) => void) => PiCustomRenderer,
		options?: { overlay?: boolean },
	): Promise<T | null>;
	theme: PiTheme;
}

export interface PiTheme {
	fg(color: string, text: string): string;
	bold(text: string): string;
	muted(text: string): string;
	dim(text: string): string;
	accent(text: string): string;
	warning(text: string): string;
}

export interface PiTUI {
	requestRender(): void;
}

export interface PiCustomRenderer {
	render(w: unknown): void;
	invalidate(): void;
	handleInput(data: string): void;
}

export interface SelectItem {
	value: string;
	label: string;
	description?: string;
}

/** Minimal retry options used across extensions. */
export interface RetryOptions {
	maxAttempts?: number;
	baseDelayMs?: number;
}
