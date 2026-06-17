use crate::tui::view_model::{ExecutionPhase, TuiViewModel};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let lines: Vec<String> = if let Some(err) = &vm.error {
        vec![
            format!("  Error: {}", err),
            "".into(),
            "  Press [Tab] to return to Dashboard,".into(),
            "  or type a new intent in the command bar.".into(),
        ]
    } else if vm.phase == ExecutionPhase::Planning {
        vec![
            "  Generating template from intent...".into(),
            "  Calling LLM to create a template definition.".into(),
            format!(
                "  Phase: {:?} | LLM calls: {}",
                vm.phase, vm.metrics.llm_calls
            ),
            "".into(),
            "  Press [Tab] to check Dashboard for progress.".into(),
        ]
    } else if let Some(tid) = &vm.template_id {
        vec![
            format!("  Generated template: {}", tid),
            format!("  Saved to .rigorix/templates/{}.toml", tid),
            format!(
                "  Status: {:?} | LLM calls: {}",
                vm.phase, vm.metrics.llm_calls
            ),
            "".into(),
            "  Press [Tab] to return to Dashboard,".into(),
            "  or type a new intent to generate another template.".into(),
        ]
    } else {
        vec![
            "  No templates loaded yet.".into(),
            "".into(),
            "  Type an intent in the command bar, then press [g] Generate".into(),
            "  to create a template from your intent using the LLM.".into(),
            "".into(),
            "  Templates are stored in .rigorix/templates/ as TOML files".into(),
            "  and can be reused across sessions.".into(),
        ]
    };
    frame.render_widget(
        List::new(
            lines
                .iter()
                .map(|s| Line::from(Span::raw(s.as_str())))
                .collect::<Vec<_>>(),
        )
        .block(
            Block::default()
                .title(" Templates ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        ),
        area,
    );
}
