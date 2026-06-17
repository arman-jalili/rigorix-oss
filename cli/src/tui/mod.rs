//! Terminal UI module — interactive TUI (ratatui).
//! @canonical .pi/architecture/modules/tui.md

pub mod command_bar;
pub mod event_bridge;
pub mod input;
pub mod orchestrator_spawner;
pub mod plan_review;
pub mod view_model;
pub mod views;
pub mod widgets;

use std::time::Duration;

use ratatui::Frame;
use ratatui::Terminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use tokio_util::sync::CancellationToken;

use crate::cli_boundary::config::CliConfig;

use self::command_bar::CommandBarState;
use self::input::keymap;
use self::input::{InputFocus, KeyAction};
use self::view_model::{ActiveView, ExecutionPhase, TuiViewModel};
use self::widgets::{LayoutMode, WidgetContext, cmd_bar, status_bar};

/// Run the interactive TUI.
pub async fn run(
    config: CliConfig,
    cancellation_token: CancellationToken,
    exec: Option<uuid::Uuid>,
    run: Option<String>,
) {
    let _ = (config, cancellation_token, exec, run);

    let _ = ratatui::crossterm::terminal::enable_raw_mode();
    let _ = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::EnterAlternateScreen,
        ratatui::crossterm::event::EnableMouseCapture,
    );
    let mut terminal =
        match Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout())) {
            Ok(t) => t,
            Err(e) => {
                restore_terminal();
                eprintln!("Failed to initialise terminal: {e}");
                return;
            }
        };

    let mut vm = TuiViewModel::default();
    let mut command_bar = CommandBarState::default();
    let mut input_focus = InputFocus::CommandBar;
    let mut selected_node: Option<String> = None;

    let result = run_event_loop(
        &mut terminal,
        &mut vm,
        &mut command_bar,
        &mut input_focus,
        &mut selected_node,
    )
    .await;

    restore_terminal();
    if let Err(e) = result {
        eprintln!("TUI error: {e}");
    }
}

fn restore_terminal() {
    let _ = ratatui::crossterm::terminal::disable_raw_mode();
    let _ = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::LeaveAlternateScreen,
        ratatui::crossterm::event::DisableMouseCapture,
    );
}

async fn run_event_loop(
    terminal: &mut Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>>,
    vm: &mut TuiViewModel,
    command_bar: &mut CommandBarState,
    input_focus: &mut InputFocus,
    selected_node: &mut Option<String>,
) -> Result<(), String> {
    loop {
        terminal
            .draw(|frame| {
                let area = frame.area();
                let layout_mode = LayoutMode::from_size(area.width, area.height);
                let ctx = WidgetContext {
                    area,
                    layout_mode,
                    color_enabled: true,
                    detailed: layout_mode != LayoutMode::Compact,
                };
                draw_frame(
                    frame,
                    area,
                    &ctx,
                    vm,
                    command_bar,
                    input_focus,
                    selected_node,
                );
            })
            .map_err(|e| format!("Render error: {e}"))?;

        if event::poll(Duration::from_millis(50)).map_err(|e| format!("Poll error: {e}"))?
            && let Event::Key(key) = event::read().map_err(|e| format!("Read error: {e}"))?
            && (key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat)
        {
            let action = keymap::map_key(key, *input_focus);
            if !handle_action(action, vm, command_bar, input_focus, selected_node, key) {
                break;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_action(
    action: KeyAction,
    vm: &mut TuiViewModel,
    command_bar: &mut CommandBarState,
    input_focus: &mut InputFocus,
    selected_node: &mut Option<String>,
    key: event::KeyEvent,
) -> bool {
    match action {
        KeyAction::Quit => return false,
        KeyAction::FocusCommandBar => {
            *input_focus = InputFocus::CommandBar;
            command_bar.focused = true;
        }
        KeyAction::BlurCommandBar => {
            *input_focus = InputFocus::Dashboard;
            command_bar.focused = false;
        }
        KeyAction::ExecuteCommand => {
            if let Some(parsed) = command_bar.parse() {
                match parsed {
                    command_bar::CommandBarInput::Intent(intent) => {
                        vm.intent = Some(intent.clone());
                        vm.phase = ExecutionPhase::Planning;
                        vm.active_view = ActiveView::Plan;
                        *input_focus = InputFocus::PlanReview;
                        command_bar.focused = false;
                    }
                    command_bar::CommandBarInput::SlashCommand(cmd) => match cmd.as_str() {
                        "history" => vm.active_view = ActiveView::History,
                        "templates" => vm.active_view = ActiveView::Templates,
                        "audit" | "events" => vm.active_view = ActiveView::Events,
                        "nodes" => vm.active_view = ActiveView::Nodes,
                        "settings" => vm.active_view = ActiveView::Settings,
                        _ => {}
                    },
                    command_bar::CommandBarInput::ColonCommand(cmd) => match cmd.as_str() {
                        "q" => return false,
                        "cancel" | "cancel!" => vm.phase = ExecutionPhase::Cancelled,
                        _ => {}
                    },
                }
                command_bar.submit();
            }
        }
        KeyAction::NextView | KeyAction::PrevView => {
            let cycle = [
                ActiveView::Dashboard,
                ActiveView::Nodes,
                ActiveView::Events,
                ActiveView::History,
            ];
            let idx = cycle.iter().position(|v| *v == vm.active_view).unwrap_or(0);
            let len = cycle.len();
            let dir: i32 = if matches!(action, KeyAction::NextView) {
                1
            } else {
                -1
            };
            vm.active_view = cycle[((idx as i32 + dir + len as i32) as usize) % len];
        }
        KeyAction::RunPlan => {
            vm.phase = ExecutionPhase::Executing;
            vm.active_view = ActiveView::Dashboard;
        }
        KeyAction::PlanOnly => {
            vm.active_view = ActiveView::Dashboard;
        }
        KeyAction::GenerateTemplate => {
            vm.active_view = ActiveView::Templates;
        }
        KeyAction::SelectNext | KeyAction::SelectPrev => {
            let ids: Vec<String> = vm.nodes.keys().cloned().collect();
            if !ids.is_empty() {
                let idx = selected_node
                    .as_ref()
                    .and_then(|s| ids.iter().position(|id| id == s))
                    .unwrap_or(0);
                let dir: usize = if matches!(action, KeyAction::SelectNext) {
                    1
                } else {
                    ids.len() - 1
                };
                *selected_node = Some(ids[(idx + dir) % ids.len()].clone());
            }
        }
        KeyAction::ToggleExpand
        | KeyAction::ShowOutput
        | KeyAction::Scroll(_)
        | KeyAction::ShowHelp
        | KeyAction::Search
        | KeyAction::FilterEvents(_) => {}
        KeyAction::ShowDetail => {
            if selected_node.is_some() {
                vm.active_view = ActiveView::Nodes;
            }
        }
        KeyAction::CancelGraceful | KeyAction::CancelImmediate => {
            vm.phase = ExecutionPhase::Cancelled;
        }
        KeyAction::None => {
            if *input_focus == InputFocus::CommandBar
                && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            {
                handle_command_bar_key(command_bar, key);
            }
        }
    }
    true
}

fn handle_command_bar_key(state: &mut CommandBarState, key: event::KeyEvent) {
    use event::KeyCode;
    match key.code {
        KeyCode::Char(c) if !key.modifiers.contains(event::KeyModifiers::CONTROL) => {
            state.text.insert(state.cursor, c);
            state.cursor += 1;
        }
        KeyCode::Backspace => {
            if state.cursor > 0 {
                state.cursor -= 1;
                state.text.remove(state.cursor);
            }
        }
        KeyCode::Delete => {
            if state.cursor < state.text.len() {
                state.text.remove(state.cursor);
            }
        }
        KeyCode::Left => state.cursor = state.cursor.saturating_sub(1),
        KeyCode::Right => state.cursor = (state.cursor + 1).min(state.text.len()),
        KeyCode::Up => state.history_up(),
        KeyCode::Down => state.history_down(),
        KeyCode::Home => state.cursor = 0,
        KeyCode::End => state.cursor = state.text.len(),
        _ => {}
    }
}

fn draw_frame(
    frame: &mut Frame<'_>,
    area: Rect,
    ctx: &WidgetContext,
    vm: &TuiViewModel,
    command_bar: &CommandBarState,
    input_focus: &InputFocus,
    selected_node: &Option<String>,
) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);
    status_bar::render(frame, chunks[0], vm);
    render_main_content(frame, chunks[1], ctx, vm, selected_node);
    cmd_bar::render(
        frame,
        chunks[2],
        command_bar,
        *input_focus == InputFocus::CommandBar,
    );
}

fn render_main_content(
    frame: &mut Frame<'_>,
    area: Rect,
    ctx: &WidgetContext,
    vm: &TuiViewModel,
    selected_node: &Option<String>,
) {
    match vm.active_view {
        ActiveView::Dashboard => views::dashboard::render(frame, area, ctx, vm, selected_node),
        ActiveView::Plan => views::plan::render(frame, area, vm),
        ActiveView::History => views::history::render(frame, area, vm),
        ActiveView::Events => views::events::render(frame, area, vm),
        ActiveView::Nodes => views::nodes::render(frame, area, vm, selected_node),
        ActiveView::Settings => views::settings::render(frame, area),
        ActiveView::Templates => views::templates::render(frame, area, vm),
        ActiveView::Clarification => views::clarification::render(frame, area),
        ActiveView::Diff => views::diff::render(frame, area, vm),
    }
}
