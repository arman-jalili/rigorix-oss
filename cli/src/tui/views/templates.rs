use crate::tui::view_model::TuiViewModel;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, vm: &TuiViewModel) {
    let lines: Vec<String> = if vm.template_id.is_some() {
        vec![
            format!(
                "  Active template: {}",
                vm.template_id.as_deref().unwrap_or("(none)")
            ),
            "  Templates are stored in .rigorix/templates/".into(),
            "".into(),
            "  Available commands:".into(),
            "    /templates      List all templates".into(),
            "    /generate       Create a new template from intent".into(),
            "    :template show  <id>  Show template details".into(),
        ]
    } else {
        vec![
            "  No templates loaded yet.".into(),
            "".into(),
            "  Run an intent with [g] Generate to create a template,".into(),
            "  or type `/generate` in the command bar.".into(),
            "".into(),
            "  Templates are stored in .rigorix/templates/ as YAML files".into(),
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
