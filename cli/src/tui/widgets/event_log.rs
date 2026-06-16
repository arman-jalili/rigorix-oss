use crate::tui::view_model::TuiViewModel;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let entries: Vec<Line> = vm
        .event_log
        .iter()
        .rev()
        .take(50)
        .map(|e| {
            let c = match e.event_type.as_str() {
                t if t.contains("Error") || t.contains("Failed") => Color::Red,
                t if t.contains("Completed") => Color::Green,
                t if t.contains("Started") || t.contains("Running") => Color::Cyan,
                _ => Color::White,
            };
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", &e.event_type[..e.event_type.len().min(20)]),
                    Style::default().fg(c),
                ),
                Span::raw(&e.summary),
            ])
        })
        .collect();
    frame.render_widget(
        List::new(entries).block(
            Block::default()
                .title(" Events ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
}
