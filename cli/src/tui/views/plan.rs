use crate::tui::view_model::TuiViewModel;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let intent = vm.intent.as_deref().unwrap_or("(no intent)");
    let template = vm.template_id.as_deref().unwrap_or("(detecting...)");
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(" Intent: ", Style::default().bold()),
            Span::raw(intent),
        ]),
        Line::from(vec![
            Span::styled(" Template: ", Style::default().bold()),
            Span::raw(template),
        ]),
        Line::from(vec![
            Span::styled(" Nodes: ", Style::default().bold()),
            Span::raw(vm.nodes.len().to_string()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [r] Run", Style::default().fg(Color::Green)),
            Span::raw("    "),
            Span::styled("[p] Plan Only", Style::default().fg(Color::Yellow)),
            Span::raw("    "),
            Span::styled("[g] Generate", Style::default().fg(Color::Blue)),
            Span::raw("    "),
            Span::styled("[Esc] Cancel", Style::default().fg(Color::Red)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Plan Preview ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}
