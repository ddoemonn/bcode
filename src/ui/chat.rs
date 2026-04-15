use crate::app::App;
use crate::provider::Role;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use super::theme;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" chat ")
        .borders(Borders::ALL)
        .border_style(theme::border(theme::BORDER));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.messages.iter().all(|m| matches!(m.role, Role::System)) && app.streaming_text.is_empty() {
        render_welcome(f, inner);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        match msg.role {
            Role::System => continue,
            Role::User => {
                let text = msg.content.text();
                if text.is_empty() { continue; }
                lines.push(Line::from(vec![
                    Span::styled("you  ", theme::label(theme::USER)),
                    Span::raw(text.to_string()),
                ]));
                lines.push(Line::raw(""));
            }
            Role::Assistant => {
                let text = msg.content.text();
                if text.is_empty() { continue; }
                lines.push(Line::from(Span::styled("ai", theme::label(theme::AI))));
                for line in crate::markdown::to_lines(text) {
                    let mut indented = vec![Span::raw("     ")];
                    indented.extend(line.spans);
                    lines.push(Line::from(indented));
                }
                lines.push(Line::raw(""));
            }
        }
    }

    if !app.streaming_text.is_empty() {
        lines.push(Line::from(Span::styled("ai", theme::label(theme::AI))));
        let md_lines = crate::markdown::to_lines(&app.streaming_text);
        let md_count = md_lines.len();
        for (i, line) in md_lines.into_iter().enumerate() {
            let mut indented = vec![Span::raw("     ")];
            indented.extend(line.spans);
            if i == md_count - 1 {
                indented.push(Span::styled("▋", theme::label(theme::AI)));
            }
            lines.push(Line::from(indented));
        }
    }

    let effective_scroll = if app.auto_scroll { u16::MAX } else { app.scroll };

    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((effective_scroll, 0)),
        inner,
    );
}

fn render_welcome(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::raw(""),
        Line::raw(""),
        Line::raw(""),
        Line::from(Span::styled(
            "  bcode",
            Style::default()
                .fg(ratatui::style::Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("  terminal ai coding agent", theme::dim())),
        Line::raw(""),
        Line::from(Span::styled(
            "  ─────────────────────────────",
            theme::dim(),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  enter        ", theme::dim()),
            Span::styled("send message", theme::subtle()),
        ]),
        Line::from(vec![
            Span::styled("  ctrl+r       ", theme::dim()),
            Span::styled("session browser", theme::subtle()),
        ]),
        Line::from(vec![
            Span::styled("  ctrl+c       ", theme::dim()),
            Span::styled("interrupt / quit", theme::subtle()),
        ]),
        Line::from(vec![
            Span::styled("  ↑ / ↓        ", theme::dim()),
            Span::styled("history / scroll", theme::subtle()),
        ]),
        Line::from(vec![
            Span::styled("  /help        ", theme::dim()),
            Span::styled("all slash commands", theme::subtle()),
        ]),
        Line::raw(""),
        Line::from(Span::styled("  type a message to start", theme::dim())),
    ];

    f.render_widget(Paragraph::new(Text::from(lines)), area);
}
