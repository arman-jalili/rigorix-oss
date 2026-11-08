use crate::tui::view_model::{ExecutionPhase, TuiViewModel};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use rigorix_engine::orchestrator::application::dto::EvidenceStatus;

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
            Span::styled(evidence_label(&vm.evidence), evidence_style(&vm.evidence)),
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

/// The evidence sentence for the status bar (#1491).
///
/// One line, next to the run's verdict, because "did this run's evidence reach
/// anywhere" is part of the verdict — a completed run with a lost envelope is not
/// the same outcome as a completed run with a complete one.
pub fn evidence_label(evidence: &EvidenceStatus) -> String {
    match evidence {
        EvidenceStatus::NotRequested => "evidence: not requested".to_string(),
        EvidenceStatus::Complete => "evidence: complete".to_string(),
        EvidenceStatus::Degraded {
            reason,
            undelivered,
        } => format!("evidence: degraded — {reason} ({undelivered} undelivered)"),
    }
}

/// `complete` is quiet, `degraded` is not: the one state an operator must notice.
pub fn evidence_style(evidence: &EvidenceStatus) -> Style {
    match evidence {
        EvidenceStatus::Degraded { .. } => {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        }
        EvidenceStatus::Complete => Style::default().fg(Color::Green),
        EvidenceStatus::NotRequested => Style::default().fg(Color::DarkGray),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1491: the **rendered sentence**, not the struct. Dropping the field from
    /// the renderer makes this fail.
    #[test]
    fn a_degraded_run_says_why_and_how_much_is_missing() {
        let evidence = EvidenceStatus::Degraded {
            reason: "the backend refused the envelope".to_string(),
            undelivered: 2,
        };
        let label = evidence_label(&evidence);
        assert!(label.contains("degraded"), "{label}");
        assert!(
            label.contains("the backend refused the envelope"),
            "{label}"
        );
        assert!(label.contains("2 undelivered"), "{label}");
        assert_eq!(evidence_style(&evidence).fg, Some(Color::Red));
    }

    #[test]
    fn a_complete_run_stays_quiet() {
        let label = evidence_label(&EvidenceStatus::Complete);
        assert_eq!(label, "evidence: complete");
        assert!(!label.contains("degraded"));
        assert_ne!(
            evidence_style(&EvidenceStatus::Complete).fg,
            Some(Color::Red)
        );
    }

    #[test]
    fn an_audit_disabled_run_says_so_instead_of_looking_complete() {
        // "Not requested" and "complete" must not render the same: the whole point
        // of #1480 was that an unwired audit path looked like success.
        assert_ne!(
            evidence_label(&EvidenceStatus::NotRequested),
            evidence_label(&EvidenceStatus::Complete)
        );
    }
}
