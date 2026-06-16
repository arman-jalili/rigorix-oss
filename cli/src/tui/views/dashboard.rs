use crate::tui::view_model::TuiViewModel;
use crate::tui::widgets::{self, WidgetContext};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

pub fn render(
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
    widgets::dag_tree::render(frame, chunks[0], vm);

    if detailed && chunks.len() > 2 {
        let node = selected_node.as_ref().and_then(|id| vm.nodes.get(id));
        widgets::node_detail::render(frame, chunks[1], node);
        let right = Layout::vertical([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(1),
        ])
        .split(chunks[2]);
        widgets::progress_bar::render(frame, right[0], vm);
        widgets::metrics_panel::render(frame, right[1], vm);
        widgets::event_log::render(frame, right[2], vm);
    } else {
        let right = Layout::vertical([Constraint::Length(8), Constraint::Min(1)]).split(chunks[1]);
        widgets::metrics_panel::render(frame, right[0], vm);
        widgets::event_log::render(frame, right[1], vm);
    }
}
