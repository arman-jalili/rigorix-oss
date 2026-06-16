use crate::tui::command_bar::CommandBarState;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &CommandBarState, focused: bool) {
    let style = if focused {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let input = if state.text.is_empty() && focused {
        " Type an intent or /command...".into()
    } else {
        state.text.clone()
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(if focused { " ▸ " } else { "   " }, style),
            Span::styled(input, style),
        ]))
        .block(
            Block::default()
                .title(" Command ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if focused {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
        ),
        area,
    );
}
