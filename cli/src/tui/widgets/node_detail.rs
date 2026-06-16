use crate::tui::view_model::NodeViewModel;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, node: Option<&NodeViewModel>) {
    let text = match node {
        Some(n) => {
            let status = format!("{:?}", n.status);
            let out = n.output_preview.as_deref().unwrap_or("(no output)");
            let err = n.error.as_deref().unwrap_or("(no error)");
            format!(
                "Name:     {}\nTool:     {}\nStatus:   {}\nRetries:  {}\nRisk:     {}\n\nOutput:\n  {}\n\nError:\n  {}",
                n.name,
                n.tool_name,
                status,
                n.retry_count,
                n.risk_level.as_deref().unwrap_or("unknown"),
                out,
                err
            )
        }
        None => "Select a node to view details".to_string(),
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .title(" Node Detail ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}
