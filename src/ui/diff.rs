use crate::app::{App, Status};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use super::{permission, theme};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let (title, border_color) = match &app.status {
        Status::AwaitingPermission => (" permission ", theme::BORDER_WARN),
        Status::Executing => (" running ", theme::BORDER_INFO),
        Status::Streaming => (" tool output ", theme::BORDER),
        _ => (" diff ", theme::BORDER),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::border(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = if app.status == Status::AwaitingPermission {
        permission::render_content(app)
    } else if app.diff_content.is_empty() {
        Text::from(Line::from(Span::styled(
            "no output yet",
            Style::default().fg(theme::DIM),
        )))
    } else {
        Text::from(
            app.diff_content
                .lines()
                .map(|l| {
                    if l.starts_with("+ ") || l.starts_with("+\t") {
                        Line::from(Span::styled(l.to_string(), Style::default().fg(theme::DIFF_ADD)))
                    } else if l.starts_with("- ") || l.starts_with("-\t") {
                        Line::from(Span::styled(l.to_string(), Style::default().fg(theme::DIFF_REM)))
                    } else if l.starts_with("@@") {
                        Line::from(Span::styled(l.to_string(), Style::default().fg(theme::DIFF_HUNK)))
                    } else if l.starts_with("tool:") {
                        Line::from(Span::styled(l.to_string(), Style::default().fg(theme::TOOL)))
                    } else if l.starts_with("---") || l.starts_with("+++") {
                        Line::from(Span::styled(l.to_string(), theme::dim()))
                    } else {
                        Line::from(l.to_string())
                    }
                })
                .collect::<Vec<_>>(),
        )
    };

    f.render_widget(Paragraph::new(content).wrap(Wrap { trim: false }), inner);
}
