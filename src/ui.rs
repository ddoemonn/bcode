use crate::app::{App, Status};
use crate::provider::Role;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(outer[0]);

    render_chat(f, app, panes[0]);
    render_diff(f, app, panes[1]);
    render_status(f, app, outer[1]);
    render_input(f, app, outer[2]);
}

fn render_chat(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = Block::default()
        .title(" Chat ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        match msg.role {
            Role::User => {
                lines.push(Line::from(vec![
                    Span::styled("you  ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw(msg.content.clone()),
                ]));
                lines.push(Line::raw(""));
            }
            Role::Assistant => {
                lines.push(Line::from(vec![
                    Span::styled("ai   ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw(msg.content.clone()),
                ]));
                lines.push(Line::raw(""));
            }
            Role::System => {}
        }
    }

    if !app.streaming_text.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("ai   ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(app.streaming_text.clone(), Style::default().fg(Color::White)),
            Span::styled("▋", Style::default().fg(Color::Green)),
        ]));
    }

    let effective_scroll = if app.auto_scroll { u16::MAX } else { app.scroll };

    let para = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((effective_scroll, 0));

    f.render_widget(para, inner);
}

fn render_diff(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = Block::default()
        .title(" Diff ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = if app.diff_content.is_empty() {
        Text::from(vec![Line::from(Span::styled(
            "no changes yet",
            Style::default().fg(Color::DarkGray),
        ))])
    } else {
        let lines: Vec<Line> = app.diff_content.lines().map(|l| {
            if l.starts_with('+') {
                Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Green)))
            } else if l.starts_with('-') {
                Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Red)))
            } else if l.starts_with('@') {
                Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Cyan)))
            } else {
                Line::from(l.to_string())
            }
        }).collect();
        Text::from(lines)
    };

    let para = Paragraph::new(content).wrap(Wrap { trim: false });
    f.render_widget(para, inner);
}

fn render_status(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let total = app.tokens.input + app.tokens.output;

    let status = match &app.status {
        Status::Ready => Span::styled("● ready", Style::default().fg(Color::Green)),
        Status::Streaming => Span::styled("◎ streaming", Style::default().fg(Color::Yellow)),
        Status::Error(_) => Span::styled("✗ error", Style::default().fg(Color::Red)),
    };

    let model = Span::styled(
        format!(" {} / {}  │", app.provider.name(), app.provider.model()),
        Style::default().fg(Color::DarkGray),
    );

    let tokens = Span::styled(
        format!("  tokens {}/{}  │  ", fmt_k(total), fmt_k(app.tokens.max)),
        Style::default().fg(Color::DarkGray),
    );

    let err = if let Status::Error(ref e) = app.status {
        Span::styled(format!("  {e}"), Style::default().fg(Color::Red))
    } else {
        Span::raw("")
    };

    let para = Paragraph::new(Line::from(vec![model, tokens, status, err]))
        .style(Style::default().bg(Color::Black));

    f.render_widget(para, area);
}

fn render_input(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let streaming = app.status == Status::Streaming;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if streaming {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Blue)
        });

    let inner = block.inner(area);
    f.render_widget(block, area);

    let prompt = Span::styled("> ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD));

    let body = if streaming {
        Span::styled("ctrl+c to interrupt", Style::default().fg(Color::DarkGray))
    } else {
        Span::raw(app.input.clone())
    };

    f.render_widget(Paragraph::new(Line::from(vec![prompt, body])), inner);

    if !streaming {
        let x = (inner.x + 2 + app.cursor_pos as u16).min(inner.x + inner.width.saturating_sub(1));
        f.set_cursor_position((x, inner.y));
    }
}

fn fmt_k(n: u32) -> String {
    if n >= 1_000 {
        format!("{:.1}k", n as f32 / 1_000.0)
    } else {
        n.to_string()
    }
}
