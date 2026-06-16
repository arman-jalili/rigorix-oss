//! Terminal UI module — interactive TUI (ratatui).
//!
//! @canonical .pi/architecture/modules/tui.md
//! Implements: TUI module — full render loop with widget-based views

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
use ratatui::style::{Color, Modifier, Style, Stylize};
use tokio_util::sync::CancellationToken;

use crate::cli_boundary::config::CliConfig;

use self::command_bar::CommandBarState;
use self::input::keymap;
use self::input::{InputFocus, KeyAction};
use self::view_model::{ActiveView, ExecutionPhase, NodeStatus, TuiViewModel};
use self::widgets::{LayoutMode, WidgetContext};

/// Run the interactive TUI.
pub async fn run(
    config: CliConfig,
    cancellation_token: CancellationToken,
    exec: Option<uuid::Uuid>,
    run: Option<String>,
) {
    let _ = (config, cancellation_token, exec, run);

    // Initialise terminal using crossterm
    let _ = ratatui::crossterm::terminal::enable_raw_mode();
    let _ = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::EnterAlternateScreen,
        ratatui::crossterm::event::EnableMouseCapture
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

    // Initialise state
    let mut vm = TuiViewModel::default();
    let mut command_bar = CommandBarState::default();
    let mut input_focus = InputFocus::CommandBar;
    let mut selected_node: Option<String> = None;

    // Run event loop
    let result = run_event_loop(
        &mut terminal,
        &mut vm,
        &mut command_bar,
        &mut input_focus,
        &mut selected_node,
    )
    .await;

    // Restore terminal
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
        ratatui::crossterm::event::DisableMouseCapture
    );
}

/// Main TUI event loop.
async fn run_event_loop(
    terminal: &mut Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>>,
    vm: &mut TuiViewModel,
    command_bar: &mut CommandBarState,
    input_focus: &mut InputFocus,
    selected_node: &mut Option<String>,
) -> Result<(), String> {
    loop {
        // Render frame
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

        // Handle input with timeout for responsive cancellation
        if event::poll(Duration::from_millis(100)).map_err(|e| format!("Poll error: {e}"))?
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

/// Handle a key action. Returns false if the TUI should quit.
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
                    }
                    command_bar::CommandBarInput::SlashCommand(cmd) => match cmd.as_str() {
                        "history" => vm.active_view = ActiveView::History,
                        "templates" => vm.active_view = ActiveView::Templates,
                        "audit" | "events" => vm.active_view = ActiveView::Events,
                        "nodes" => vm.active_view = ActiveView::Nodes,
                        "settings" => vm.active_view = ActiveView::Settings,
                        "help" => { /* show help overlay — future */ }
                        _ => {}
                    },
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

        KeyAction::NextView => {
            let cycle = [
                ActiveView::Dashboard,
                ActiveView::Nodes,
                ActiveView::Events,
                ActiveView::History,
            ];
            let idx = cycle.iter().position(|v| *v == vm.active_view).unwrap_or(0);
            vm.active_view = cycle[(idx + 1) % cycle.len()];
        }
        KeyAction::PrevView => {
            let cycle = [
                ActiveView::Dashboard,
                ActiveView::Nodes,
                ActiveView::Events,
                ActiveView::History,
            ];
            let idx = cycle.iter().position(|v| *v == vm.active_view).unwrap_or(0);
            vm.active_view = cycle[(idx + cycle.len() - 1) % cycle.len()];
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

        KeyAction::SelectNext => {
            let ids: Vec<String> = vm.nodes.keys().cloned().collect();
            if !ids.is_empty() {
                let idx = selected_node
                    .as_ref()
                    .and_then(|s| ids.iter().position(|id| id == s))
                    .unwrap_or(0);
                *selected_node = Some(ids[(idx + 1) % ids.len()].clone());
            }
        }
        KeyAction::SelectPrev => {
            let ids: Vec<String> = vm.nodes.keys().cloned().collect();
            if !ids.is_empty() {
                let idx = selected_node
                    .as_ref()
                    .and_then(|s| ids.iter().position(|id| id == s))
                    .unwrap_or(0);
                *selected_node = Some(ids[(idx + ids.len() - 1) % ids.len()].clone());
            }
        }

        KeyAction::ToggleExpand => {}
        KeyAction::ShowDetail => {
            if selected_node.is_some() {
                vm.active_view = ActiveView::Nodes;
            }
        }
        KeyAction::ShowOutput => {}
        KeyAction::Scroll(_) => {}
        KeyAction::CancelGraceful => {
            vm.phase = ExecutionPhase::Cancelled;
        }
        KeyAction::CancelImmediate => {
            vm.phase = ExecutionPhase::Cancelled;
        }
        KeyAction::ShowHelp => {}
        KeyAction::Search => {}
        KeyAction::FilterEvents(_) => {}

        KeyAction::None => {
            // If command bar focused, handle text input
            if *input_focus == InputFocus::CommandBar
                && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            {
                handle_command_bar_key(command_bar, key);
            }
        }
    }
    true
}

/// Handle keyboard input when the command bar has focus.
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

/// Draw the complete TUI frame.
fn draw_frame(
    frame: &mut Frame<'_>,
    area: Rect,
    ctx: &WidgetContext,
    vm: &TuiViewModel,
    command_bar: &CommandBarState,
    input_focus: &InputFocus,
    selected_node: &Option<String>,
) {
    // Layout: status bar (1 line) + main content (fills) + command bar (3 lines)
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);

    // Status bar
    widgets::render_status_bar(frame, chunks[0], vm);

    // Main content area (based on active view)
    render_main_content(frame, chunks[1], ctx, vm, selected_node);

    // Command bar
    widgets::render_command_bar(
        frame,
        chunks[2],
        command_bar,
        *input_focus == InputFocus::CommandBar,
    );
}

/// Render the main content area based on active view.
fn render_main_content(
    frame: &mut Frame<'_>,
    area: Rect,
    ctx: &WidgetContext,
    vm: &TuiViewModel,
    selected_node: &Option<String>,
) {
    match vm.active_view {
        ActiveView::Dashboard => render_dashboard_view(frame, area, ctx, vm, selected_node),
        ActiveView::Plan => render_plan_view(frame, area, vm),
        ActiveView::History => render_history_view(frame, area, vm),
        ActiveView::Events => render_events_view(frame, area, vm),
        ActiveView::Nodes => render_nodes_view(frame, area, vm, selected_node),
        ActiveView::Settings => render_settings_view(frame, area),
        ActiveView::Templates => render_templates_view(frame, area, vm),
        ActiveView::Clarification => render_clarification_view(frame, area),
        ActiveView::Diff => render_diff_view(frame, area, vm),
    }
}

/// Render the dashboard view with DAG tree + details + metrics.
fn render_dashboard_view(
    frame: &mut Frame<'_>,
    area: Rect,
    ctx: &WidgetContext,
    vm: &TuiViewModel,
    selected_node: &Option<String>,
) {
    let detailed = ctx.detailed;

    let chunks = if detailed {
        Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
        ])
        .split(area)
    } else {
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).split(area)
    };

    // Left: DAG tree
    widgets::render_dag_tree(frame, chunks[0], vm);

    if detailed && chunks.len() > 2 {
        // Center: node details
        let node = selected_node.as_ref().and_then(|id| vm.nodes.get(id));
        widgets::render_node_detail(frame, chunks[1], node);

        // Right: metrics + budget
        let right_chunks = Layout::vertical([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(1),
        ])
        .split(chunks[2]);

        widgets::render_progress_bar(frame, right_chunks[0], vm);
        widgets::render_metrics_panel(frame, right_chunks[1], vm);
        widgets::render_event_log(frame, right_chunks[2], vm);
    } else {
        // Compact: details only
        let right_chunks =
            Layout::vertical([Constraint::Length(8), Constraint::Min(1)]).split(chunks[1]);
        widgets::render_metrics_panel(frame, right_chunks[0], vm);
        widgets::render_event_log(frame, right_chunks[1], vm);
    }
}

/// Render the plan preview view.
fn render_plan_view(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let intent = vm.intent.as_deref().unwrap_or("(no intent)");
    let template = vm.template_id.as_deref().unwrap_or("(detecting...)");
    let node_count = vm.nodes.len();

    let lines = vec![
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(" Intent: ", ratatui::style::Style::default().bold()),
            ratatui::text::Span::raw(intent),
        ]),
        ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(" Template: ", ratatui::style::Style::default().bold()),
            ratatui::text::Span::raw(template),
        ]),
        ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(" Nodes: ", ratatui::style::Style::default().bold()),
            ratatui::text::Span::raw(node_count.to_string()),
        ]),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                " [r] Run",
                ratatui::style::Style::default().fg(Color::Green),
            ),
            ratatui::text::Span::raw("    "),
            ratatui::text::Span::styled(
                "[p] Plan Only",
                ratatui::style::Style::default().fg(Color::Yellow),
            ),
            ratatui::text::Span::raw("    "),
            ratatui::text::Span::styled(
                "[g] Generate",
                ratatui::style::Style::default().fg(Color::Blue),
            ),
            ratatui::text::Span::raw("    "),
            ratatui::text::Span::styled(
                "[Esc] Cancel",
                ratatui::style::Style::default().fg(Color::Red),
            ),
        ]),
    ];

    let block = ratatui::widgets::Block::default()
        .title(" Plan Preview ")
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(Color::Cyan));
    let paragraph = ratatui::widgets::Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render the history view.
fn render_history_view(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let entries: Vec<ratatui::text::Line> = if vm.command_bar_history.is_empty() {
        vec![ratatui::text::Line::from(" No previous commands.")]
    } else {
        vm.command_bar_history
            .iter()
            .enumerate()
            .map(|(i, cmd)| ratatui::text::Line::from(format!("  {}. {cmd}", i + 1)))
            .collect()
    };

    let list = ratatui::widgets::List::new(entries).block(
        ratatui::widgets::Block::default()
            .title(" Command History ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(ratatui::style::Style::default().fg(Color::Blue)),
    );
    frame.render_widget(list, area);
}

/// Render the events view.
fn render_events_view(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let entries: Vec<ratatui::text::Line> = if vm.event_log.is_empty() {
        vec![ratatui::text::Line::from(" No events.")]
    } else {
        vm.event_log
            .iter()
            .rev()
            .take(100)
            .map(|e| {
                let color = match e.event_type.as_str() {
                    t if t.contains("Error") || t.contains("Failed") => Color::Red,
                    t if t.contains("Completed") => Color::Green,
                    t if t.contains("Started") || t.contains("Running") => Color::Cyan,
                    _ => Color::White,
                };
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(
                        format!("[{}] ", &e.event_type[..e.event_type.len().min(15)]),
                        ratatui::style::Style::default().fg(color),
                    ),
                    ratatui::text::Span::raw(&e.summary),
                ])
            })
            .collect()
    };

    let list = ratatui::widgets::List::new(entries).block(
        ratatui::widgets::Block::default()
            .title(" Events ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(ratatui::style::Style::default().fg(Color::Magenta)),
    );
    frame.render_widget(list, area);
}

/// Render the nodes detail view.
fn render_nodes_view(
    frame: &mut Frame<'_>,
    area: Rect,
    vm: &TuiViewModel,
    selected_node: &Option<String>,
) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

    // Left: node list
    let items: Vec<ratatui::widgets::ListItem> = vm
        .nodes
        .values()
        .map(|n| {
            let icon = match n.status {
                NodeStatus::Completed => "✓",
                NodeStatus::InProgress => "▶",
                NodeStatus::Failed => "✗",
                NodeStatus::Retrying => "↻",
                NodeStatus::Pending => "·",
                NodeStatus::Skipped => "–",
            };
            let selected = selected_node.as_deref() == Some(&n.id);
            let style = if selected {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };
            ratatui::widgets::ListItem::new(ratatui::text::Line::from(vec![
                ratatui::text::Span::raw(format!(" {} ", icon)),
                ratatui::text::Span::raw(&n.name),
            ]))
            .style(style)
        })
        .collect();

    let list = ratatui::widgets::List::new(items).block(
        ratatui::widgets::Block::default()
            .title(" All Nodes ")
            .borders(ratatui::widgets::Borders::ALL),
    );
    frame.render_widget(list, chunks[0]);

    // Right: selected node details
    let node = selected_node.as_ref().and_then(|id| vm.nodes.get(id));
    widgets::render_node_detail(frame, chunks[1], node);
}

/// Render the Settings view — configuration options.
fn render_settings_view(frame: &mut Frame<'_>, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(4), Constraint::Min(1)]).split(area);

    let header = ratatui::widgets::Paragraph::new(ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(
            " Rigorix Configuration ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL));
    frame.render_widget(header, chunks[0]);

    let items = vec![
        "  Output Format    --format pretty|json|markdown|quiet",
        "  Verbosity        -v (debug), -vv (trace)",
        "  Log Filter       RIGORIX_LOG env var (default: info)",
        "  Config File      rigorix.toml (CWD) or ~/.rigorix/config.toml",
        "  Parallel Tasks   orchestrator.max_parallel_tasks (default: 4)",
        "  Max Retries      orchestrator.max_retries (default: 3)",
        "  Timeout          orchestrator.default_timeout_secs (default: 120)",
        "  LLM Provider     llm.provider (anthropic|openai|deepseek)",
        "  LLM Model        llm.model (default: claude-sonnet-4-6)",
        "  Audit Enabled    audit.enabled (default: false)",
        "",
        "  Run `rigorix config show` to see current values",
        "  Run `rigorix config validate` to check configuration",
    ];
    let lines: Vec<ratatui::text::Line> = items
        .iter()
        .map(|s| ratatui::text::Line::from(ratatui::text::Span::raw(*s)))
        .collect();

    let list = ratatui::widgets::List::new(lines).block(
        ratatui::widgets::Block::default()
            .title(" Settings ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(list, chunks[1]);
}

/// Render the Templates view — available templates.
fn render_templates_view(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let templates = if vm.template_id.is_some() {
        vec![
            format!(
                "  Active template: {}",
                vm.template_id.as_deref().unwrap_or("(none)")
            ),
            "  Templates listed here are stored in .rigorix/templates/".to_string(),
            "".to_string(),
            "  Available commands:".to_string(),
            "    /templates      List all templates".to_string(),
            "    /generate       Create a new template from intent".to_string(),
            "    :template show  <id>  Show template details".to_string(),
        ]
    } else {
        vec![
            "  No templates loaded yet.".to_string(),
            "".to_string(),
            "  Run an intent with [g] Generate to create a template,".to_string(),
            "  or type `/generate` in the command bar.".to_string(),
            "".to_string(),
            "  Templates are stored in .rigorix/templates/ as YAML files".to_string(),
            "  and can be reused across sessions.".to_string(),
        ]
    };

    let lines: Vec<ratatui::text::Line> = templates
        .iter()
        .map(|s| ratatui::text::Line::from(ratatui::text::Span::raw(s.as_str())))
        .collect();

    let list = ratatui::widgets::List::new(lines).block(
        ratatui::widgets::Block::default()
            .title(" Templates ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Green)),
    );
    frame.render_widget(list, area);
}

/// Render the Clarification view — LLM clarification requests.
fn render_clarification_view(frame: &mut Frame<'_>, area: Rect) {
    let items = vec![
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "  LLM Clarification Requests",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(ratatui::text::Span::raw(
            "  When the LLM needs more information to generate a plan,",
        )),
        ratatui::text::Line::from(ratatui::text::Span::raw(
            "  it will ask clarifying questions here.",
        )),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "  Example questions:",
            Style::default().fg(Color::Blue),
        )),
        ratatui::text::Line::from(ratatui::text::Span::raw(
            "    • Which authentication method should be used? (OAuth2 / API key / JWT)",
        )),
        ratatui::text::Line::from(ratatui::text::Span::raw(
            "    • Should the new endpoint be public or require authentication?",
        )),
        ratatui::text::Line::from(ratatui::text::Span::raw(
            "    • What database backend should be used? (PostgreSQL / SQLite)",
        )),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "  Type your answer in the command bar and press Enter.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = ratatui::widgets::Paragraph::new(items)
        .block(
            ratatui::widgets::Block::default()
                .title(" Clarification ")
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render the Diff view — plan comparison side-by-side.
fn render_diff_view(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

    // Left side (current/older plan)
    let left_nodes: Vec<ratatui::text::Line> = if vm.nodes.is_empty() {
        vec![ratatui::text::Line::from(ratatui::text::Span::styled(
            "  (no plan loaded)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        vm.nodes
            .values()
            .take(20)
            .map(|n| {
                let icon = match n.status {
                    NodeStatus::Completed => "✓",
                    NodeStatus::InProgress => "▶",
                    NodeStatus::Failed => "✗",
                    NodeStatus::Retrying => "↻",
                    _ => "·",
                };
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::raw(format!("  {} ", icon)),
                    ratatui::text::Span::raw(&n.name),
                    ratatui::text::Span::styled(
                        format!(" ({})", n.tool_name),
                        Style::default().fg(Color::Blue),
                    ),
                ])
            })
            .collect()
    };

    let left_list = ratatui::widgets::List::new(left_nodes).block(
        ratatui::widgets::Block::default()
            .title(" Plan A ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(left_list, chunks[0]);

    // Right side (diff/newer plan)
    let right_lines = vec![
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "  Diff view shows changes between two plans.",
            Style::default().fg(Color::DarkGray),
        )),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(ratatui::text::Span::raw(
            "  Use `:diff <id1> <id2>` to compare two executions,",
        )),
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "  or press [d] on a plan preview to diff against the last.",
            Style::default().fg(Color::Yellow),
        )),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "  Changes are highlighted:",
            Style::default().fg(Color::Green),
        )),
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "    + Added nodes",
            Style::default().fg(Color::Green),
        )),
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "    - Removed nodes",
            Style::default().fg(Color::Red),
        )),
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "    ~ Modified nodes",
            Style::default().fg(Color::Yellow),
        )),
    ];

    let right_para = ratatui::widgets::Paragraph::new(right_lines).block(
        ratatui::widgets::Block::default()
            .title(" Diff ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(right_para, chunks[1]);
}
