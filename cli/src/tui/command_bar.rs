//! Command bar — persistent text input at the bottom of the screen.
//!
//! @canonical .pi/architecture/modules/tui.md#command-bar
//! Implements: Contract Freeze — CommandBar component
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! The command bar is a persistent text input at the bottom of the screen.
//! Users type intents, `/commands`, and `:commands` here.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Command bar state
// ---------------------------------------------------------------------------

/// Parsed command from the command bar input.
#[derive(Debug, Clone)]
pub enum CommandBarInput {
    /// A natural-language intent to plan/execute.
    Intent(String),
    /// A slash command: e.g. /history, /templates, /help
    SlashCommand(String),
    /// A colon command: e.g. :q, :cancel, :cancel!
    ColonCommand(String),
}

/// State of the command bar text input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandBarState {
    /// Current text in the input field.
    pub text: String,
    /// Cursor position within the text.
    pub cursor: usize,
    /// Command history (previous submitted inputs).
    pub history: Vec<String>,
    /// Current position in history navigation (-1 = new input).
    pub history_index: i64,
    /// Whether the command bar has focus.
    pub focused: bool,
    /// Autocomplete suggestions (shown when typing /).
    pub suggestions: Vec<String>,
    /// Selected suggestion index.
    pub selected_suggestion: Option<usize>,
}

impl Default for CommandBarState {
    fn default() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: -1,
            focused: true,
            suggestions: Vec::new(),
            selected_suggestion: None,
        }
    }
}

impl CommandBarState {
    /// Parse the current text as a `CommandBarInput`.
    pub fn parse(&self) -> Option<CommandBarInput> {
        if self.text.starts_with('/') {
            Some(CommandBarInput::SlashCommand(self.text[1..].to_string()))
        } else if self.text.starts_with(':') {
            Some(CommandBarInput::ColonCommand(self.text[1..].to_string()))
        } else if !self.text.is_empty() {
            Some(CommandBarInput::Intent(self.text.clone()))
        } else {
            None
        }
    }

    /// Push the current text to history and clear.
    pub fn submit(&mut self) {
        if !self.text.is_empty() {
            self.history.push(self.text.clone());
            self.text.clear();
            self.cursor = 0;
            self.history_index = -1;
        }
    }

    /// Navigate backwards through command history.
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = if self.history_index < 0 {
            self.history.len() as i64 - 1
        } else {
            (self.history_index - 1).max(0)
        };
        self.history_index = idx;
        self.text = self.history[idx as usize].clone();
        self.cursor = self.text.len();
    }

    /// Navigate forwards through command history.
    pub fn history_down(&mut self) {
        if self.history_index < 0 {
            return;
        }
        if self.history_index as usize >= self.history.len() - 1 {
            self.history_index = -1;
            self.text.clear();
            self.cursor = 0;
        } else {
            self.history_index += 1;
            self.text = self.history[self.history_index as usize].clone();
            self.cursor = self.text.len();
        }
    }
}
