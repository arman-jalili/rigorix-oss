use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, Paragraph},
};
pub fn render(frame: &mut Frame<'_>, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(4), Constraint::Min(1)]).split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            " Rigorix Configuration ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]))
        .block(Block::default().borders(Borders::ALL)),
        chunks[0],
    );
    let items = vec![
        "  Output Format    --format pretty|json|markdown|quiet",
        "  Verbosity        -v (debug), -vv (trace)",
        "  Log Filter       RIGORIX_LOG env var (default: info)",
        "  Config File      rigorix.toml (CWD) or ~/.rigorix/config.toml",
        "  Parallel Tasks   orchestrator.max_parallel_tasks (default: 4)",
        "  Max Retries      orchestrator.max_retries (default: 3)",
        "  Timeout          orchestrator.default_timeout_secs (default: 120)",
        "  LLM Provider     llm.provider (anthropic|openai|deepseek)",
        "  LLM Model        llm.model (default: claude-sonnet-4-6)",
        "  Audit Enabled    audit.enabled (default: false)",
        "",
        "  Run `rigorix config show` to see current values",
        "  Run `rigorix config validate` to check configuration",
    ];
    frame.render_widget(
        List::new(
            items
                .iter()
                .map(|s| Line::from(Span::raw(*s)))
                .collect::<Vec<_>>(),
        )
        .block(
            Block::default()
                .title(" Settings ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        chunks[1],
    );
}
