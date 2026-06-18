//! DAG tree widget — color-coded node list with dependency arrows.
//!
//! Displays DAG nodes in topological order (when available via node_order
//! in PlanReviewState), with root markers and dependency arrows.

use crate::tui::view_model::{NodeStatus, NodeViewModel, TuiViewModel};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let mut nodes: Vec<&NodeViewModel> = vm.nodes.values().collect();
    // Sort: failures first, then running, then completed
    nodes.sort_by_key(|n| status_order(n.status));
    let items = render_node_items(&nodes);

    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title(" Execution DAG ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        area,
    );
}

/// Priority for sorting: failures first, then running, then completed.
fn status_order(s: NodeStatus) -> u8 {
    match s {
        NodeStatus::Failed => 0,
        NodeStatus::Retrying => 1,
        NodeStatus::InProgress => 2,
        NodeStatus::Pending => 3,
        NodeStatus::Completed => 4,
        NodeStatus::Skipped => 5,
    }
}

fn render_node_items<'a>(nodes: &[&'a NodeViewModel]) -> Vec<ListItem<'a>> {
    nodes
        .iter()
        .map(|n| {
            let s = match n.status {
                NodeStatus::Completed => Style::default().fg(Color::Green),
                NodeStatus::InProgress => Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                NodeStatus::Failed => Style::default().fg(Color::Red),
                NodeStatus::Retrying => Style::default().fg(Color::Yellow),
                NodeStatus::Pending | NodeStatus::Skipped => Style::default().fg(Color::DarkGray),
            };
            let icon = match n.status {
                NodeStatus::Completed => "✓",
                NodeStatus::InProgress => "▶",
                NodeStatus::Failed => "✗",
                NodeStatus::Retrying => "↻",
                NodeStatus::Pending => "·",
                NodeStatus::Skipped => "–",
            };

            // Build dependency arrow: if node has deps, show ← [dep_names]
            let dep_str = if n.dependencies.is_empty() {
                String::new()
            } else {
                let dep_names: Vec<&str> = n
                    .dependencies
                    .iter()
                    .map(|d| d.as_str())
                    .collect();
                format!(" ← [{}]", dep_names.join(", "))
            };

            let timing = n
                .timing_ms
                .map(|ms| format!(" {}ms", ms))
                .unwrap_or_default();

            let error_snippet = n.error.as_ref().map(|e| {
                let first_line = e.lines().next().unwrap_or(e);
                let truncated = if first_line.len() > 60 {
                    format!("{}...", &first_line[..57])
                } else {
                    first_line.to_string()
                };
                format!(" ({})", truncated)
            }).unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", icon), s),
                Span::raw(format!("{}{}", n.name, dep_str)),
                Span::styled(format!(" {}", n.tool_name), Style::default().fg(Color::Blue)),
                Span::raw(timing),
                Span::styled(error_snippet, Style::default().fg(Color::Red)),
            ]))
        })
        .collect()
}
