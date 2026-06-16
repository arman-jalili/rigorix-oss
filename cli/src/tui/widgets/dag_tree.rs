//! DAG tree widget — color-coded node list.
use crate::tui::view_model::{NodeStatus, NodeViewModel, TuiViewModel};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let nodes: Vec<&NodeViewModel> = vm.nodes.values().collect();
    let items: Vec<ListItem> = nodes
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
            let timing = n
                .timing_ms
                .map(|ms| format!(" {}ms", ms))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", icon), s),
                Span::raw(format!(" {} ", n.name)),
                Span::styled(n.tool_name.clone(), Style::default().fg(Color::Blue)),
                Span::raw(timing),
            ]))
        })
        .collect();
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
