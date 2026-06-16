use crate::tui::view_model::{NodeStatus, TuiViewModel};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, Paragraph},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    let left: Vec<Line> = if vm.nodes.is_empty() {
        vec![Line::from(Span::styled(
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
                Line::from(vec![
                    Span::raw(format!("  {} ", icon)),
                    Span::raw(&n.name),
                    Span::styled(
                        format!(" ({})", n.tool_name),
                        Style::default().fg(Color::Blue),
                    ),
                ])
            })
            .collect()
    };
    frame.render_widget(
        List::new(left).block(
            Block::default()
                .title(" Plan A ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "  Diff view shows changes between two plans.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::raw(
                "  Use :diff <id1> <id2> to compare, or press [d] on a plan preview.",
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Changes are highlighted:",
                Style::default().fg(Color::Green),
            )),
            Line::from(Span::styled(
                "    + Added nodes",
                Style::default().fg(Color::Green),
            )),
            Line::from(Span::styled(
                "    - Removed nodes",
                Style::default().fg(Color::Red),
            )),
            Line::from(Span::styled(
                "    ~ Modified nodes",
                Style::default().fg(Color::Yellow),
            )),
        ])
        .block(
            Block::default()
                .title(" Diff ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        chunks[1],
    );
}
