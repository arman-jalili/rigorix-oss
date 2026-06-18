//! Keymap — keyboard binding configuration.
//!
//! @canonical .pi/architecture/modules/tui.md#key-bindings
//! Implements: InputHandler component — real key mapping

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{InputFocus, KeyAction};

/// Map a `KeyEvent` to a `KeyAction` based on the current focus mode.
pub fn map_key(event: KeyEvent, focus: InputFocus) -> KeyAction {
    use KeyCode::*;

    // ── Global bindings (work regardless of focus) ──────────────────
    match event.code {
        Char('q')
            if !event.modifiers.contains(KeyModifiers::CONTROL)
                && focus != InputFocus::CommandBar =>
        {
            return KeyAction::Quit;
        }
        F(1) => return KeyAction::ShowHelp,
        Tab => return KeyAction::NextView,
        Esc => {
            return match focus {
                InputFocus::CommandBar => KeyAction::BlurCommandBar,
                _ => KeyAction::FocusCommandBar,
            };
        }
        _ => {}
    }

    // Ctrl+C handling (any focus)
    if event.code == Char('c') && event.modifiers.contains(KeyModifiers::CONTROL) {
        return KeyAction::CancelGraceful;
    }
    // Ctrl+Y: copy current view to clipboard (any focus)
    if event.code == Char('y') && event.modifiers.contains(KeyModifiers::CONTROL) {
        return KeyAction::CopyToClipboard;
    }

    // ── Focus-specific bindings ─────────────────────────────────────
    match focus {
        InputFocus::CommandBar => map_command_bar(event),
        InputFocus::PlanReview => map_plan_review(event),
        InputFocus::Dashboard => map_dashboard(event),
        InputFocus::Events => map_events(event),
        InputFocus::View(_) => KeyAction::None,
    }
}

/// Keys for Command Bar focus mode.
fn map_command_bar(event: KeyEvent) -> KeyAction {
    match event.code {
        KeyCode::Enter => KeyAction::ExecuteCommand,
        KeyCode::Up => KeyAction::SelectPrev,
        KeyCode::Down => KeyAction::SelectNext,
        KeyCode::Tab => KeyAction::None, // handled globally, pass through
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            KeyAction::CancelGraceful
        }
        // All other keys are text input — handled by render loop directly
        _ => KeyAction::None,
    }
}

/// Keys for Plan Preview focus mode.
fn map_plan_review(event: KeyEvent) -> KeyAction {
    match event.code {
        KeyCode::Char('r') | KeyCode::Enter => KeyAction::RunPlan,
        KeyCode::Char('p') => KeyAction::PlanOnly,
        KeyCode::Char('g') => KeyAction::GenerateTemplate,
        KeyCode::Char('d') => KeyAction::ShowDetail,
        KeyCode::Char('e') => KeyAction::Search, // "edit" parameters
        KeyCode::Up => KeyAction::SelectPrev,
        KeyCode::Down => KeyAction::SelectNext,
        KeyCode::Esc => KeyAction::BlurCommandBar, // cancel → back to command bar
        _ => KeyAction::None,
    }
}

/// Keys for Dashboard focus mode.
fn map_dashboard(event: KeyEvent) -> KeyAction {
    match event.code {
        KeyCode::Up => KeyAction::SelectPrev,
        KeyCode::Down => KeyAction::SelectNext,
        KeyCode::Enter => KeyAction::ToggleExpand,
        KeyCode::Char('d') => KeyAction::ShowDetail,
        KeyCode::Char('o') => KeyAction::ShowOutput,
        KeyCode::Char(' ') => KeyAction::ToggleExpand,
        KeyCode::Char('h') => KeyAction::PrevView,
        KeyCode::Char('l') => KeyAction::NextView,
        KeyCode::Esc => KeyAction::FocusCommandBar,
        _ => KeyAction::None,
    }
}

/// Keys for Events view focus mode.
fn map_events(event: KeyEvent) -> KeyAction {
    match event.code {
        KeyCode::Char('j') | KeyCode::Down => KeyAction::Scroll(1),
        KeyCode::Char('k') | KeyCode::Up => KeyAction::Scroll(-1),
        KeyCode::Char('g') => KeyAction::Scroll(i32::MIN), // top
        KeyCode::Char('G') => KeyAction::Scroll(i32::MAX), // bottom
        KeyCode::Char('/') => KeyAction::Search,
        KeyCode::Char('1') => KeyAction::FilterEvents(1),
        KeyCode::Char('2') => KeyAction::FilterEvents(2),
        KeyCode::Char('3') => KeyAction::FilterEvents(3),
        KeyCode::Char('4') => KeyAction::FilterEvents(4),
        KeyCode::Char('5') => KeyAction::FilterEvents(5),
        KeyCode::Esc => KeyAction::FocusCommandBar,
        _ => KeyAction::None,
    }
}
