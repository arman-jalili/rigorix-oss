//! Terminal UI module — interactive TUI (ratatui).
//!
//! @canonical .pi/architecture/modules/tui.md
//! Implements: TUI module — render loop with ratatui
//! Issue: issue-renderer, issue-views, issue-inputhandler

pub mod command_bar;
pub mod event_bridge;
pub mod input;
pub mod orchestrator_spawner;
pub mod plan_review;
pub mod view_model;
pub mod views;
pub mod widgets;

use std::time::Duration;

use ratatui::Terminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use tokio_util::sync::CancellationToken;

use crate::cli_boundary::config::CliConfig;

use self::command_bar::CommandBarState;
use self::input::keymap;
use self::input::{InputFocus, KeyAction};
use self::view_model::{ActiveView, ExecutionPhase, TuiViewModel};
use self::widgets::LayoutMode;

/// Run the interactive TUI.
pub async fn run(
    config: CliConfig,
    cancellation_token: CancellationToken,
    exec: Option<uuid::Uuid>,
    run: Option<String>,
) {
    let _ = (config, cancellation_token, exec, run);

    // Initialise terminal using crossterm
    let stdout = std::io::stdout();
    let _ = ratatui::crossterm::terminal::enable_raw_mode();
    let mut terminal = match ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to initialise terminal: {e}");
            let _ = ratatui::crossterm::terminal::disable_raw_mode();
            return;
        }
    };

    // Initialise state
    let mut vm = TuiViewModel::default();
    let mut command_bar = CommandBarState::default();
    let mut input_focus = InputFocus::CommandBar;

    // Run event loop
    let result = run_event_loop(&mut terminal, &mut vm, &mut command_bar, &mut input_focus).await;

    // Restore terminal
    let _ = ratatui::crossterm::terminal::disable_raw_mode();
    let _ = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::LeaveAlternateScreen,
        ratatui::crossterm::event::DisableMouseCapture
    );
    if let Err(e) = result {
        eprintln!("TUI error: {e}");
    }
}

/// Main TUI event loop.
async fn run_event_loop(
    terminal: &mut Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>>,
    vm: &mut TuiViewModel,
    command_bar: &mut CommandBarState,
    input_focus: &mut InputFocus,
) -> Result<(), String> {
    loop {
        // Render frame
        terminal
            .draw(|frame| {
                let area = frame.area();
                let layout_mode = LayoutMode::from_size(area.width, area.height);
                render_frame(frame, area, layout_mode, vm, command_bar, input_focus);
            })
            .map_err(|e| format!("Render error: {e}"))?;

        // Handle input with timeout for responsive cancellation
        if event::poll(Duration::from_millis(100)).map_err(|e| format!("Poll error: {e}"))?
            && let Event::Key(key) = event::read().map_err(|e| format!("Read error: {e}"))?
            && (key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat)
        {
            let action = keymap::map_key(key, *input_focus);
            if !handle_action(action, vm, command_bar, input_focus, key) {
                break; // Quit
            }
        }
    }
    Ok(())
}

/// Handle a key action. Returns false if the TUI should quit.
fn handle_action(
    action: KeyAction,
    vm: &mut TuiViewModel,
    command_bar: &mut CommandBarState,
    input_focus: &mut InputFocus,
    key: event::KeyEvent,
) -> bool {
    use KeyAction::*;

    match action {
        Quit => return false,

        FocusCommandBar => {
            *input_focus = InputFocus::CommandBar;
            command_bar.focused = true;
        }
        BlurCommandBar => {
            *input_focus = InputFocus::Dashboard;
            command_bar.focused = false;
        }
        ExecuteCommand => {
            if let Some(parsed) = command_bar.parse() {
                match parsed {
                    command_bar::CommandBarInput::Intent(intent) => {
                        vm.intent = Some(intent.clone());
                        vm.phase = ExecutionPhase::Planning;
                        vm.active_view = ActiveView::Plan;
                    }
                    command_bar::CommandBarInput::SlashCommand(cmd) => {
                        match cmd.as_str() {
                            "history" => vm.active_view = ActiveView::History,
                            "templates" => vm.active_view = ActiveView::Templates,
                            "audit" => vm.active_view = ActiveView::Events,
                            "help" => { /* show help overlay */ }
                            _ => {}
                        }
                    }
                    command_bar::CommandBarInput::ColonCommand(cmd) => match cmd.as_str() {
                        "q" => return false,
                        "cancel" => vm.phase = ExecutionPhase::Cancelled,
                        "cancel!" => vm.phase = ExecutionPhase::Cancelled,
                        _ => {}
                    },
                }
                command_bar.submit();
            }
        }
        NextView | PrevView => {
            let views = [
                ActiveView::Dashboard,
                ActiveView::Nodes,
                ActiveView::Events,
                ActiveView::History,
            ];
            let idx = views.iter().position(|v| *v == vm.active_view).unwrap_or(0);
            vm.active_view = views[(idx + 1) % views.len()];
        }

        // Plan preview actions
        RunPlan => {
            vm.phase = ExecutionPhase::Executing;
            vm.active_view = ActiveView::Dashboard;
        }
        PlanOnly => {
            vm.active_view = ActiveView::Dashboard;
        }
        GenerateTemplate => {
            vm.active_view = ActiveView::Templates;
        }

        // Scroll / navigation
        SelectNext => {}
        SelectPrev => {}
        Scroll(_) => {}
        ToggleExpand => {}
        ShowDetail => {}
        ShowOutput => {}

        // Command bar text input
        _ if *input_focus == InputFocus::CommandBar => {
            handle_command_bar_key(command_bar, key);
        }

        None | CancelGraceful | CancelImmediate | ShowHelp | Search | FilterEvents(_) => {}
    }
    true
}

/// Handle keyboard input when the command bar has focus.
fn handle_command_bar_key(state: &mut CommandBarState, key: event::KeyEvent) {
    use event::KeyCode;
    match key.code {
        KeyCode::Char(c) => {
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
        KeyCode::Left => {
            state.cursor = state.cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            state.cursor = state.cursor.min(state.text.len());
        }
        KeyCode::Up => state.history_up(),
        KeyCode::Down => state.history_down(),
        KeyCode::Home => state.cursor = 0,
        KeyCode::End => state.cursor = state.text.len(),
        _ => {}
    }
}

/// Render the entire TUI frame.
fn render_frame(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    layout_mode: LayoutMode,
    vm: &TuiViewModel,
    command_bar: &CommandBarState,
    input_focus: &InputFocus,
) {
    let has_status_bar = true;
    let bar_height = if has_status_bar { 3 } else { 1 };

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(bar_height)]).split(area);

    // Main content area
    render_main_content(frame, chunks[0], layout_mode, vm);

    // Status bar + command bar
    render_bottom_bar(frame, chunks[1], command_bar, input_focus, vm);
}

/// Render the main content area based on active view.
fn render_main_content(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    layout_mode: LayoutMode,
    vm: &TuiViewModel,
) {
    let _ = layout_mode;

    match vm.active_view {
        ActiveView::Dashboard => render_dashboard(frame, area, vm),
        ActiveView::Plan => render_plan_preview(frame, area, vm),
        ActiveView::History => render_history(frame, area, vm),
        ActiveView::Events => render_events(frame, area, vm),
        ActiveView::Nodes => render_nodes(frame, area, vm),
        ActiveView::Settings => render_settings(frame, area),
        ActiveView::Templates => render_templates(frame, area),
        ActiveView::Clarification => render_placeholder(frame, area, "Clarification"),
        ActiveView::Diff => render_placeholder(frame, area, "Diff"),
    }
}

/// Render a placeholder view.
fn render_placeholder(frame: &mut ratatui::Frame<'_>, area: Rect, name: &str) {
    let block = Block::default()
        .title(format!(" {name} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new(format!("{name} view — implementation pending"))
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(text, area);
}

/// Render the dashboard view (DAG tree + details + metrics).
fn render_dashboard(frame: &mut ratatui::Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(6),
    ])
    .split(area);

    // Header
    let phase_str = format!("{:?}", vm.phase);
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" Rigorix ", Style::default().fg(Color::Cyan).bold()),
        Span::raw(" — "),
        Span::styled(phase_str, Style::default().fg(Color::Yellow)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, chunks[0]);

    // Node list / details area
    let node_chunks = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let node_list = if vm.nodes.is_empty() {
        Paragraph::new(" No execution loaded. Type an intent in the command bar.")
            .block(Block::default().title(" Nodes ").borders(Borders::ALL))
    } else {
        let node_text: Vec<String> = vm
            .nodes
            .values()
            .map(|n| format!("  {} [{}]", n.name, status_char(n.status)))
            .collect();
        Paragraph::new(node_text.join("\n"))
            .block(Block::default().title(" Nodes ").borders(Borders::ALL))
    };
    frame.render_widget(node_list, node_chunks[0]);

    // Details panel
    let details = Paragraph::new(" Select a node to view details")
        .block(Block::default().title(" Details ").borders(Borders::ALL));
    frame.render_widget(details, node_chunks[1]);

    // Metrics bar
    let metrics_text = format!(
        " LLM calls: {} | Tokens: {} | Nodes: {}/{} | Throughput: {:.1}/s",
        vm.metrics.llm_calls,
        vm.metrics.tokens,
        vm.metrics.nodes_completed,
        vm.metrics.nodes_total,
        vm.metrics.throughput,
    );
    let metrics = Paragraph::new(metrics_text)
        .block(Block::default().title(" Metrics ").borders(Borders::ALL));
    frame.render_widget(metrics, chunks[2]);
}

/// Render the plan preview view.
fn render_plan_preview(frame: &mut ratatui::Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let intent = vm.intent.as_deref().unwrap_or("(no intent)");
    let template = vm.template_id.as_deref().unwrap_or("(detecting...)");

    let text = format!(
        "\n  Intent: {intent}\n  Template: {template}\n\n  [r] Run    [p] Plan Only    [g] Generate    [Esc] Cancel\n"
    );
    let preview = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Plan Preview ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, area);
}

/// Render the history view.
fn render_history(frame: &mut ratatui::Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let entries = if vm.command_bar_history.is_empty() {
        " No previous commands.".to_string()
    } else {
        vm.command_bar_history
            .iter()
            .enumerate()
            .map(|(i, cmd)| format!("  {}. {cmd}", i + 1))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let text =
        Paragraph::new(entries).block(Block::default().title(" History ").borders(Borders::ALL));
    frame.render_widget(text, area);
}

/// Render the events view.
fn render_events(frame: &mut ratatui::Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let entries = if vm.event_log.is_empty() {
        " No events.".to_string()
    } else {
        vm.event_log
            .iter()
            .map(|e| format!("  [{}] {} — {}", e.timestamp_ms, e.event_type, e.summary))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let text =
        Paragraph::new(entries).block(Block::default().title(" Events ").borders(Borders::ALL));
    frame.render_widget(text, area);
}

/// Render the nodes view.
fn render_nodes(frame: &mut ratatui::Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let entries = if vm.nodes.is_empty() {
        " No nodes.".to_string()
    } else {
        vm.nodes
            .values()
            .map(|n| {
                format!(
                    "  {} | {} | tool={} | retries={}",
                    status_char(n.status),
                    n.name,
                    n.tool_name,
                    n.retry_count
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let text =
        Paragraph::new(entries).block(Block::default().title(" Nodes ").borders(Borders::ALL));
    frame.render_widget(text, area);
}

/// Render the settings view.
fn render_settings(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let text = Paragraph::new(" Settings view — implementation pending")
        .block(Block::default().title(" Settings ").borders(Borders::ALL));
    frame.render_widget(text, area);
}

/// Render the templates view.
fn render_templates(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let text = Paragraph::new(" Templates view — implementation pending")
        .block(Block::default().title(" Templates ").borders(Borders::ALL));
    frame.render_widget(text, area);
}

/// Render the bottom bar (status + command input).
fn render_bottom_bar(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    command_bar: &CommandBarState,
    input_focus: &InputFocus,
    vm: &TuiViewModel,
) {
    let chunks = Layout::horizontal([Constraint::Length(20), Constraint::Min(1)]).split(area);

    // Status info
    let phase_str = format!(" {:?} ", vm.phase);
    let status_line = Paragraph::new(Line::from(vec![
        Span::raw(phase_str),
        Span::raw(format!(" | {} nodes", vm.nodes.len())),
    ]));
    frame.render_widget(status_line, chunks[0]);

    // Command bar text input
    let prefix = if command_bar.focused { ">" } else { " " };
    let cmd_text = format!(" {prefix} {}", command_bar.text);
    let cmd_style = if *input_focus == InputFocus::CommandBar {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let cmd_line = Paragraph::new(cmd_text)
        .style(cmd_style)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(cmd_line, chunks[1]);
}

/// Return a single character representing the node status.
fn status_char(status: view_model::NodeStatus) -> char {
    use view_model::NodeStatus::*;
    match status {
        Pending => '·',
        InProgress => '▶',
        Completed => '✓',
        Failed => '✗',
        Retrying => '↻',
        Skipped => '–',
    }
}
