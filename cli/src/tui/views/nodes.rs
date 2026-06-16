use crate::tui::view_model::{NodeStatus, TuiViewModel};
use crate::tui::widgets;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    vm: &TuiViewModel,
    selected_node: &Option<String>,
) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    let items: Vec<ListItem> = vm
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
            let sel = selected_node.as_deref() == Some(&n.id);
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {} ", icon)),
                Span::raw(&n.name),
            ]))
            .style(if sel {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            })
        })
        .collect();
    frame.render_widget(
        List::new(items).block(Block::default().title(" All Nodes ").borders(Borders::ALL)),
        chunks[0],
    );
    let node = selected_node.as_ref().and_then(|id| vm.nodes.get(id));
    widgets::node_detail::render(frame, chunks[1], node);
}
