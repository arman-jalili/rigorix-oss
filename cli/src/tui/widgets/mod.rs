//! Widget implementations — reusable ratatui UI components.
//!
//! @canonical .pi/architecture/modules/tui.md#widgets
//! Implements: Renderer component — real ratatui widgets

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap};

use crate::tui::command_bar::CommandBarState;
use crate::tui::view_model::{self, NodeStatus, NodeViewModel, TuiViewModel};

/// Terminal size mode for adaptive layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Compact,
    Standard,
    Full,
}

impl LayoutMode {
    pub fn from_size(width: u16, height: u16) -> Self {
        if width >= 160 && height >= 40 {
            LayoutMode::Full
        } else if width >= 120 && height >= 30 {
            LayoutMode::Standard
        } else {
            LayoutMode::Compact
        }
    }
}

/// Context passed to widgets during rendering.
#[derive(Debug, Clone)]
pub struct WidgetContext {
    pub area: Rect,
    pub layout_mode: LayoutMode,
    pub color_enabled: bool,
    pub detailed: bool,
}

// ── DAG Tree Widget ───────────────────────────────────────────────────────

/// Render the DAG tree: nodes arranged by dependency layers.
pub fn render_dag_tree(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let nodes: Vec<&NodeViewModel> = vm.nodes.values().collect();
    let items: Vec<ListItem> = nodes
        .iter()
        .map(|n| {
            let status_style = match n.status {
                NodeStatus::Completed => Style::default().fg(Color::Green),
                NodeStatus::InProgress => Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                NodeStatus::Failed => Style::default().fg(Color::Red),
                NodeStatus::Retrying => Style::default().fg(Color::Yellow),
                NodeStatus::Pending => Style::default().fg(Color::DarkGray),
                NodeStatus::Skipped => Style::default().fg(Color::DarkGray),
            };
            let icon = match n.status {
                NodeStatus::Completed => "✓",
                NodeStatus::InProgress => "▶",
                NodeStatus::Failed => "✗",
                NodeStatus::Retrying => "↻",
                NodeStatus::Pending => "·",
                NodeStatus::Skipped => "–",
            };
            let timing = n
                .timing_ms
                .map(|ms| format!(" {}ms", ms))
                .unwrap_or_default();
            let line = Line::from(vec![
                Span::styled(format!(" {} ", icon), status_style),
                Span::raw(format!(" {} ", n.name)),
                Span::styled(n.tool_name.clone(), Style::default().fg(Color::Blue)),
                Span::raw(timing),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Execution DAG ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, area);
}

// ── Node Detail Widget ─────────────────────────────────────────────────────

/// Render details for a selected node.
pub fn render_node_detail(frame: &mut Frame<'_>, area: Rect, node: Option<&NodeViewModel>) {
    let text = match node {
        Some(n) => {
            let status_str = format!("{:?}", n.status);
            let output = n.output_preview.as_deref().unwrap_or("(no output)");
            let error = n.error.as_deref().unwrap_or("(no error)");
            format!(
                "Name:     {}\nTool:     {}\nStatus:   {}\nRetries:  {}\nRisk:     {}\n\nOutput:\n  {}\n\nError:\n  {}",
                n.name,
                n.tool_name,
                status_str,
                n.retry_count,
                n.risk_level.as_deref().unwrap_or("unknown"),
                output,
                error,
            )
        }
        None => "Select a node to view details".to_string(),
    };

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Node Detail ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

// ── Progress Bar Widget ────────────────────────────────────────────────────

/// Render an execution progress bar with percentage.
pub fn render_progress_bar(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let total = vm.metrics.nodes_total.max(1);
    let completed = vm.metrics.nodes_completed;
    let pct = (completed as f64 / total as f64) * 100.0;
    let label = format!("{:.0}% ({}/{})", pct, completed, total);

    let gauge = Gauge::default()
        .block(Block::default().title(" Progress ").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .percent(pct as u16)
        .label(label);
    frame.render_widget(gauge, area);
}

// ── Status Bar Widget ──────────────────────────────────────────────────────

/// Render the status bar with execution phase and node count.
pub fn render_status_bar(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let phase_str = format!("{:?}", vm.phase);
    let phase_color = match vm.phase {
        view_model::ExecutionPhase::Idle => Color::DarkGray,
        view_model::ExecutionPhase::Planning => Color::Yellow,
        view_model::ExecutionPhase::Executing => Color::Cyan,
        view_model::ExecutionPhase::Completed => Color::Green,
        view_model::ExecutionPhase::Failed => Color::Red,
        view_model::ExecutionPhase::Cancelled => Color::Red,
    };

    let line = Line::from(vec![
        Span::styled(
            " RIGORIX ",
            Style::default().fg(Color::White).bg(Color::Blue),
        ),
        Span::raw(" "),
        Span::styled(
            phase_str,
            Style::default()
                .fg(phase_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            " | {} nodes | {} LLM calls",
            vm.nodes.len(),
            vm.metrics.llm_calls
        )),
    ]);
    let paragraph = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

// ── Event Log Widget ───────────────────────────────────────────────────────

/// Render the scrollable event log.
pub fn render_event_log(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let entries: Vec<Line> = vm
        .event_log
        .iter()
        .rev()
        .take(50) // show last 50 events
        .map(|e| {
            let color = match e.event_type.as_str() {
                t if t.contains("Error") || t.contains("Failed") => Color::Red,
                t if t.contains("Completed") => Color::Green,
                t if t.contains("Started") || t.contains("Running") => Color::Cyan,
                _ => Color::White,
            };
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", &e.event_type[..e.event_type.len().min(20)]),
                    Style::default().fg(color),
                ),
                Span::raw(&e.summary),
            ])
        })
        .collect();

    let list = List::new(entries).block(
        Block::default()
            .title(" Events ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(list, area);
}

// ── Command Bar Widget ─────────────────────────────────────────────────────

/// Render the command bar at the bottom of the screen.
pub fn render_command_bar(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &CommandBarState,
    focused: bool,
) {
    let prefix = if focused { "▸" } else { " " };
    let input_text = if state.text.is_empty() && focused {
        " Type an intent or /command...".to_string()
    } else {
        state.text.clone()
    };

    let style = if focused {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let cursor_style = if focused {
        Style::default().bg(Color::Green).fg(Color::Black)
    } else {
        Style::default()
    };

    let spans = vec![
        Span::styled(format!(" {} ", prefix), style),
        Span::styled(input_text, style),
    ];

    let paragraph = Paragraph::new(Line::from(spans)).style(cursor_style).block(
        Block::default()
            .title(" Command ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if focused {
                Color::Green
            } else {
                Color::DarkGray
            })),
    );
    frame.render_widget(paragraph, area);
}

// ── Metrics Panel Widget ───────────────────────────────────────────────────

/// Render the metrics panel with LLM calls, tokens, throughput.
pub fn render_metrics_panel(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let lines = vec![
        Line::from(vec![
            Span::styled("LLM Calls: ", Style::default().bold()),
            Span::raw(vm.metrics.llm_calls.to_string()),
            Span::raw("  "),
            Span::styled("Tokens: ", Style::default().bold()),
            Span::raw(vm.metrics.tokens.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Nodes: ", Style::default().bold()),
            Span::raw(format!(
                "{}/{} completed  {} failed",
                vm.metrics.nodes_completed, vm.metrics.nodes_total, vm.metrics.nodes_failed
            )),
        ]),
        Line::from(vec![
            Span::styled("Throughput: ", Style::default().bold()),
            Span::raw(format!("{:.1} nodes/s", vm.metrics.throughput)),
        ]),
        Line::from(vec![
            Span::styled("Budget: ", Style::default().bold()),
            Span::raw(format!(
                "{}/{} calls  {}/{} tokens",
                vm.llm_budget.used_calls,
                vm.llm_budget
                    .max_calls
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "∞".into()),
                vm.llm_budget.used_tokens,
                vm.llm_budget
                    .max_tokens
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "∞".into()),
            )),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Metrics ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(paragraph, area);
}
