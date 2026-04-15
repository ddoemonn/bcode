use crate::app::{App, Status};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use super::theme;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(26)])
        .split(area);

    render_left(f, app, chunks[0]);
    render_meter(f, app, chunks[1]);
}

fn render_left(f: &mut Frame, app: &App, area: Rect) {
    let status_span = match &app.status {
        Status::Ready => Span::styled("● ready", Style::default().fg(theme::AI)),
        Status::Streaming => Span::styled("◎ streaming", Style::default().fg(theme::TOOL)),
        Status::AwaitingPermission => {
            Span::styled("? permission", Style::default().fg(theme::TOOL))
        }
        Status::Executing => Span::styled("⚙ executing", Style::default().fg(theme::BORDER_INFO)),
        Status::SessionBrowser => {
            Span::styled("⊞ sessions", Style::default().fg(theme::BORDER_INFO))
        }
        Status::Setup(_) => Span::styled("◌ setup", Style::default().fg(theme::BORDER_INFO)),
        Status::Error(_) => Span::styled("✗ error", Style::default().fg(theme::ERROR)),
    };

    let provider_model = format!(
        " {}/{} ",
        app.provider.name(),
        app.provider.model()
    );

    let err_text = if let Status::Error(ref e) = app.status {
        format!("  {e}")
    } else {
        String::new()
    };

    let line = Line::from(vec![
        Span::styled(provider_model, Style::default().fg(theme::DIM)),
        Span::styled("│ ", Style::default().fg(theme::BORDER)),
        status_span,
        Span::styled(err_text, Style::default().fg(theme::ERROR)),
    ]);

    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Rgb(18, 18, 22))),
        area,
    );
}

fn render_meter(f: &mut Frame, app: &App, area: Rect) {
    let total = app.tokens.input + app.tokens.output;
    let max = app.tokens.max.max(1);
    let pct = ((total as f32 / max as f32) * 100.0).min(100.0) as u32;

    let bar_width: usize = 10;
    let filled = ((pct as f32 / 100.0) * bar_width as f32).round() as usize;
    let empty = bar_width - filled.min(bar_width);

    let bar_color = if pct >= 85 {
        theme::ERROR
    } else if pct >= 60 {
        theme::TOOL
    } else {
        theme::AI
    };

    let bar: String = format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(empty)
    );

    let line = Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme::BORDER)),
        Span::styled(bar, Style::default().fg(bar_color)),
        Span::styled(
            format!(" {pct}%"),
            Style::default().fg(theme::DIM),
        ),
    ]);

    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Rgb(18, 18, 22))),
        area,
    );
}
