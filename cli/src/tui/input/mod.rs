//! Input handling — keyboard dispatch, keymap, command palette.
//!
//! @canonical .pi/architecture/modules/tui.md#input-system
//! Implements: Contract Freeze — InputHandler component
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! The input system maps keyboard events to actions. Actions are dispatched
//! to the appropriate handler based on the current focus (command bar, plan
//! preview, dashboard, events view).

pub mod command_palette;
pub mod keymap;

use ratatui::crossterm::event::KeyEvent;

use crate::tui::view_model::ActiveView;

/// Focus mode for keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFocus {
    /// Command bar has focus — typing text.
    CommandBar,
    /// Plan preview has focus — action keys.
    PlanReview,
    /// Dashboard has focus — navigation keys.
    Dashboard,
    /// Event view has focus — scroll/search keys.
    Events,
    /// Other view has focus.
    View(ActiveView),
}

/// Action triggered by a key press.
#[derive(Debug, Clone)]
pub enum KeyAction {
    /// No action (key not bound).
    None,
    /// Focus the command bar.
    FocusCommandBar,
    /// Blur the command bar.
    BlurCommandBar,
    /// Execute the current command bar input.
    ExecuteCommand,
    /// Navigate to the next view.
    NextView,
    /// Navigate to the previous view.
    PrevView,
    /// Scroll up/down in the current view.
    Scroll(i32),
    /// Select the next/previous item.
    SelectNext,
    /// Select the previous item.
    SelectPrev,
    /// Expand/collapse the selected item.
    ToggleExpand,
    /// Show detail for the selected item.
    ShowDetail,
    /// Show full tool output.
    ShowOutput,
    /// Quit the TUI.
    Quit,
    /// Cancel the running execution (graceful).
    CancelGraceful,
    /// Cancel the running execution (immediate).
    CancelImmediate,
    /// Run the plan (from plan preview).
    RunPlan,
    /// Plan only (from plan preview).
    PlanOnly,
    /// Generate template (from plan preview).
    GenerateTemplate,
    /// Show help overlay.
    ShowHelp,
    /// Search/filter events.
    Search,
    /// Filter events by type (1-5).
    FilterEvents(u8),
}

/// Keyboard event handler trait.
///
/// Implementations map `KeyEvent` to `KeyAction` based on the current
/// focus and active view. The render loop calls this before each frame.
pub trait InputHandler: Send + Sync {
    /// Process a keyboard event and return the action to take.
    fn handle_key(&self, event: KeyEvent, focus: InputFocus) -> KeyAction;

    /// Get the current input focus.
    fn focus(&self) -> InputFocus;

    /// Set the input focus to a new mode.
    fn set_focus(&mut self, focus: InputFocus);
}
