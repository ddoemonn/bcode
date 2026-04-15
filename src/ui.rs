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
        .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(3)])
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
            Role::System => continue,
            Role::User => {
                let text = msg.content.text();
                if text.is_empty() { continue }
                lines.push(Line::from(vec![
                    Span::styled("you  ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw(text.to_string()),
                ]));
                lines.push(Line::raw(""));
            }
            Role::Assistant => {
                let text = msg.content.text();
                if text.is_empty() { continue }
                lines.push(Line::from(vec![
                    Span::styled("ai   ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw(text.to_string()),
                ]));
                lines.push(Line::raw(""));
            }
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

    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((effective_scroll, 0)),
        inner,
    );
}

fn render_diff(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let title = match &app.status {
        Status::AwaitingPermission => " Permission ",
        Status::Executing => " Running ",
        _ => " Diff ",
    };

    let border_color = match &app.status {
        Status::AwaitingPermission => Color::Yellow,
        Status::Executing => Color::Cyan,
        _ => Color::DarkGray,
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = if app.status == Status::AwaitingPermission {
        if let Some(call) = app.pending_calls.get(app.current_call_idx) {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("tool  ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(call.name.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::raw(""),
            ];
            if let Some(obj) = call.input.as_object() {
                for (k, v) in obj {
                    let val = v.as_str().unwrap_or(&v.to_string()).to_string();
                    lines.push(Line::from(vec![
                        Span::styled(format!("{k}  "), Style::default().fg(Color::DarkGray)),
                        Span::raw(val),
                    ]));
                }
            }
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("[y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" allow once   "),
                Span::styled("[n]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(" deny   "),
                Span::styled("[a]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" always"),
            ]));
            Text::from(lines)
        } else {
            Text::raw("")
        }
    } else if app.diff_content.is_empty() {
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
            } else if l.starts_with("tool:") {
                Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Yellow)))
            } else {
                Line::from(l.to_string())
            }
        }).collect();
        Text::from(lines)
    };

    f.render_widget(Paragraph::new(content).wrap(Wrap { trim: false }), inner);
}

fn render_status(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let total = app.tokens.input + app.tokens.output;

    let status_span = match &app.status {
        Status::Ready              => Span::styled("● ready",      Style::default().fg(Color::Green)),
        Status::Streaming          => Span::styled("◎ streaming",  Style::default().fg(Color::Yellow)),
        Status::AwaitingPermission => Span::styled("? permission", Style::default().fg(Color::Yellow)),
        Status::Executing          => Span::styled("⚙ executing",  Style::default().fg(Color::Cyan)),
        Status::Error(_)           => Span::styled("✗ error",      Style::default().fg(Color::Red)),
    };

    let model_span = Span::styled(
        format!(" {} / {}  │", app.provider.name(), app.provider.model()),
        Style::default().fg(Color::DarkGray),
    );

    let token_span = Span::styled(
        format!("  tokens {}/{}  │  ", fmt_k(total), fmt_k(app.tokens.max)),
        Style::default().fg(Color::DarkGray),
    );

    let err_span = if let Status::Error(ref e) = app.status {
        Span::styled(format!("  {e}"), Style::default().fg(Color::Red))
    } else {
        Span::raw("")
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![model_span, token_span, status_span, err_span]))
            .style(Style::default().bg(Color::Black)),
        area,
    );
}

fn render_input(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (border_color, hint) = match &app.status {
        Status::Streaming | Status::Executing => {
            (Color::Yellow, "ctrl+c to interrupt")
        }
        Status::AwaitingPermission => {
            (Color::Yellow, "y / n / a")
        }
        Status::Error(_) => (Color::Red, ""),
        _ => (Color::Blue, ""),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let blocked = matches!(
        app.status,
        Status::Streaming | Status::Executing | Status::AwaitingPermission
    );

    let prompt = Span::styled("> ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD));
    let body = if blocked {
        Span::styled(hint, Style::default().fg(Color::DarkGray))
    } else {
        Span::raw(app.input.clone())
    };

    f.render_widget(Paragraph::new(Line::from(vec![prompt, body])), inner);

    if !blocked {
        let x = (inner.x + 2 + app.cursor_pos as u16)
            .min(inner.x + inner.width.saturating_sub(1));
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
