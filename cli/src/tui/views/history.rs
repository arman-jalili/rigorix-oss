use crate::tui::view_model::TuiViewModel;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let entries: Vec<Line> = if vm.command_bar_history.is_empty() {
        vec![Line::from(" No previous commands.")]
    } else {
        vm.command_bar_history
            .iter()
            .enumerate()
            .map(|(i, c)| Line::from(format!("  {}. {c}", i + 1)))
            .collect()
    };
    frame.render_widget(
        List::new(entries).block(
            Block::default()
                .title(" Command History ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        ),
        area,
    );
}
