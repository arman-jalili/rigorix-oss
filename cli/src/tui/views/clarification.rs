use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
pub fn render(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "  LLM Clarification Requests",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::raw(
                "  When the LLM needs more information to generate a plan,",
            )),
            Line::from(Span::raw("  it will ask clarifying questions here.")),
            Line::from(""),
            Line::from(Span::styled(
                "  Example questions:",
                Style::default().fg(Color::Blue),
            )),
            Line::from(Span::raw(
                "    • Which authentication method? (OAuth2 / API key / JWT)",
            )),
            Line::from(Span::raw(
                "    • Should the endpoint be public or require auth?",
            )),
            Line::from(Span::raw(
                "    • What database backend? (PostgreSQL / SQLite)",
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Type your answer in the command bar and press Enter.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(
            Block::default()
                .title(" Clarification ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .wrap(Wrap { trim: false }),
        area,
    );
}
