//! Keymap — keyboard binding configuration.
//!
//! @canonical .pi/architecture/modules/tui.md#key-bindings
//! Implements: Contract Freeze — key binding configuration
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Defines the keyboard bindings for each input focus mode.

use ratatui::crossterm::event::KeyEvent;

use super::{InputFocus, KeyAction};

/// Map a `KeyEvent` to a `KeyAction` based on the current focus.
pub fn map_key(event: KeyEvent, focus: InputFocus) -> KeyAction {
    let _ = (event, focus);

    // Placeholder: returns None for all keys.
    // Implementation issue: implement full key mapping per focus mode
    // per the key bindings table in tui.md.
    KeyAction::None
}

/// Key binding table for reference during implementation.
///
/// ## Global
/// | Key | Action |
/// |-----|--------|
/// | Esc | FocusCommandBar / BlurCommandBar |
/// | Enter | ExecuteCommand |
/// | Tab | NextView |
/// | q | Quit |
/// | F1 | ShowHelp |
/// | Ctrl+C | CancelGraceful / CancelImmediate |
/// | Up/Down | Scroll / SelectPrev / SelectNext |
///
/// ## Command Bar Focused
/// | Key | Action |
/// |-----|--------|
/// | Type text | Input character |
/// | Enter | ExecuteCommand |
/// | Up/Down | Command history navigation |
/// | Tab | Autocomplete |
/// | Esc | BlurCommandBar |
///
/// ## Plan Preview
/// | Key | Action |
/// |-----|--------|
/// | r | RunPlan |
/// | p | PlanOnly |
/// | g | GenerateTemplate |
/// | d | ShowDiff |
/// | e | EditParameters |
/// | Esc | Cancel |
/// | Up/Down | Navigate nodes |
///
/// ## Dashboard
/// | Key | Action |
/// |-----|--------|
/// | Up/Down | Navigate nodes |
/// | Enter | ToggleExpand |
/// | d | ShowDetail |
/// | o | ShowOutput |
/// | Space | ToggleExpand |
///
/// ## Events
/// | Key | Action |
/// |-----|--------|
/// | j/k | Scroll |
/// | g/G | Go to top/bottom |
/// | 1-5 | FilterEvents |
/// | / | Search |
pub const fn binding_table() -> &'static str {
    "See comments above for the full key binding reference."
}
