use crate::app::App;
use crate::provider::Role;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use super::theme;

pub fn render_list(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" sessions ")
        .borders(Borders::ALL)
        .border_style(theme::border(theme::BORDER_INFO));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.session_list.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("no saved sessions", theme::dim())),
            inner,
        );
        return;
    }

    let lines: Vec<Line> = app.session_list.iter().enumerate().map(|(i, s)| {
        let selected = i == app.session_selected;
        let date = s.updated_at.format("%m/%d %H:%M").to_string();
        let max_title = inner.width.saturating_sub(14) as usize;
        let title = if s.title.len() > max_title {
            format!("{}…", &s.title[..max_title.saturating_sub(1)])
        } else {
            s.title.clone()
        };

        if selected {
            Line::from(vec![
                Span::styled(
                    format!(" ▶ {title}"),
                    Style::default()
                        .fg(ratatui::style::Color::Black)
                        .bg(theme::BORDER_INFO),
                ),
                Span::styled(
                    format!("  {date} "),
                    Style::default()
                        .fg(ratatui::style::Color::Black)
                        .bg(theme::BORDER_INFO),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(format!("   {title}"), Style::default().fg(ratatui::style::Color::White)),
                Span::styled(format!("  {date}"), theme::dim()),
            ])
        }
    }).collect();

    let scroll = (app.session_selected as u16).saturating_sub(inner.height.saturating_sub(1));
    f.render_widget(Paragraph::new(Text::from(lines)).scroll((scroll, 0)), inner);
}

pub fn render_preview(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" preview ")
        .borders(Borders::ALL)
        .border_style(theme::border(theme::BORDER));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = if let Some(meta) = app.session_list.get(app.session_selected) {
        if let Ok(s) = crate::session::load(&meta.id) {
            let lines: Vec<Line> = s
                .messages
                .iter()
                .filter_map(|m| {
                    let text = m.content.text();
                    if text.is_empty() { return None; }
                    let (label, color) = match m.role {
                        Role::User => ("you", theme::USER),
                        Role::Assistant => ("ai ", theme::AI),
                        Role::System => return None,
                    };
                    let snippet = if text.len() > 80 {
                        format!("{}…", &text[..77])
                    } else {
                        text.to_string()
                    };
                    Some(Line::from(vec![
                        Span::styled(format!("{label}  "), theme::label(color)),
                        Span::raw(snippet),
                    ]))
                })
                .collect();
            Text::from(lines)
        } else {
            Text::from(Span::styled("failed to load session", Style::default().fg(theme::ERROR)))
        }
    } else {
        Text::from(Span::styled("no session selected", theme::dim()))
    };

    f.render_widget(Paragraph::new(content).wrap(Wrap { trim: false }), inner);
}
