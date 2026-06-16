use crate::tui::view_model::TuiViewModel;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("LLM Calls: ", Style::default().bold()),
                Span::raw(vm.metrics.llm_calls.to_string()),
                Span::raw("  "),
                Span::styled("Tokens: ", Style::default().bold()),
                Span::raw(vm.metrics.tokens.to_string()),
            ]),
            Line::from(vec![
                Span::styled("Nodes: ", Style::default().bold()),
                Span::raw(format!(
                    "{}/{} completed  {} failed",
                    vm.metrics.nodes_completed, vm.metrics.nodes_total, vm.metrics.nodes_failed
                )),
            ]),
            Line::from(vec![
                Span::styled("Throughput: ", Style::default().bold()),
                Span::raw(format!("{:.1} nodes/s", vm.metrics.throughput)),
            ]),
            Line::from(vec![
                Span::styled("Budget: ", Style::default().bold()),
                Span::raw(format!(
                    "{}/{} calls  {}/{} tokens",
                    vm.llm_budget.used_calls,
                    vm.llm_budget
                        .max_calls
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "∞".into()),
                    vm.llm_budget.used_tokens,
                    vm.llm_budget
                        .max_tokens
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "∞".into())
                )),
            ]),
        ])
        .block(
            Block::default()
                .title(" Metrics ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        area,
    );
}
