use crate::tui::view_model::{ExecutionPhase, TuiViewModel};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let phase_str = format!("{:?}", vm.phase);
    let phase_color = match vm.phase {
        ExecutionPhase::Idle => Color::DarkGray,
        ExecutionPhase::Planning => Color::Yellow,
        ExecutionPhase::Executing => Color::Cyan,
        ExecutionPhase::Completed => Color::Green,
        ExecutionPhase::Failed | ExecutionPhase::Cancelled => Color::Red,
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " RIGORIX ",
                Style::default().fg(Color::White).bg(Color::Blue),
            ),
            Span::raw(" "),
            Span::styled(
                phase_str,
                Style::default()
                    .fg(phase_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                " | {} nodes | {} LLM calls",
                vm.nodes.len(),
                vm.metrics.llm_calls
            )),
            Span::raw("  "),
            Span::styled(
                vm.copy_message.as_deref().unwrap_or("[Ctrl+Y] copy"),
                Style::default().fg(if vm.copy_message.is_some() {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]))
        .block(Block::default().borders(Borders::ALL)),
        area,
    );
}
