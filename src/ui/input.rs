use crate::app::{App, Status};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::theme;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let (border_color, hint) = match &app.status {
        Status::Streaming | Status::Executing => (theme::BORDER_WARN, "ctrl+c to interrupt"),
        Status::AwaitingPermission => (theme::BORDER_WARN, "y  allow   n  deny   a  always"),
        Status::SessionBrowser => (theme::BORDER_INFO, "↑↓  navigate   enter  load   esc  back"),
        Status::Error(_) => (theme::BORDER_ERR, "ctrl+c to quit, or type to continue"),
        _ => (theme::BORDER_FOCUS, ""),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let blocked = matches!(
        app.status,
        Status::Streaming
            | Status::Executing
            | Status::AwaitingPermission
            | Status::SessionBrowser
            | Status::Setup(_)
    );

    let prompt = Span::styled("> ", theme::label(theme::BORDER_FOCUS));

    let body = if blocked {
        Span::styled(hint, Style::default().fg(theme::DIM))
    } else {
        Span::raw(app.input.clone())
    };

    f.render_widget(Paragraph::new(Line::from(vec![prompt, body])), inner);

    if !blocked {
        let x = (inner.x + 2 + app.cursor_pos as u16).min(inner.x + inner.width.saturating_sub(1));
        f.set_cursor_position((x, inner.y));
    }
}
