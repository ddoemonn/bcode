use crate::app::{App, SetupStep, Status, PROVIDERS};
use crate::provider::Role;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    if let Status::Setup(_) = &app.status {
        render_setup(f, app, area);
        return;
    }

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(3)])
        .split(area);

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(outer[0]);

    if app.status == Status::SessionBrowser {
        render_session_list(f, app, panes[0]);
        render_session_preview(f, app, panes[1]);
    } else {
        render_chat(f, app, panes[0]);
        render_diff(f, app, panes[1]);
    }

    render_status(f, app, outer[1]);
    render_input(f, app, outer[2]);
}

fn render_setup(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" bcode setup ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);

    let hint = match &app.status {
        Status::Setup(SetupStep::ChooseProvider) => "↑↓ or 1/2/3 to select,  enter to confirm",
        Status::Setup(SetupStep::EnterApiKey)    => "paste your key and press enter",
        Status::Setup(SetupStep::EnterModel)     => "edit or press enter to accept the default",
        _                                        => "",
    };

    f.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)))
            .alignment(Alignment::Center),
        rows[0],
    );

    match &app.status {
        Status::Setup(SetupStep::ChooseProvider) => render_setup_choose(f, app, rows[1]),
        Status::Setup(SetupStep::EnterApiKey)    => render_setup_key(f, app, rows[1]),
        Status::Setup(SetupStep::EnterModel)     => render_setup_model(f, app, rows[1]),
        _ => {}
    }
}

fn render_setup_choose(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![Line::raw(""), Line::raw("  Choose a provider:"), Line::raw("")];

    for (i, (_, label, model, note)) in PROVIDERS.iter().enumerate() {
        let selected = i == app.setup_selected;
        let prefix = if selected { "  ▶ " } else { "    " };
        let style = if selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{prefix}[{}] ", i + 1), style),
            Span::styled(format!("{label:<12}"), style),
            Span::styled(format!("{model:<24}"), Style::default().fg(if selected { Color::Cyan } else { Color::DarkGray })),
            Span::styled(note.to_string(), Style::default().fg(Color::DarkGray)),
        ]));
    }

    f.render_widget(Paragraph::new(Text::from(lines)), area);
}

fn render_setup_key(f: &mut Frame, app: &App, area: Rect) {
    let (_, label, _, note) = PROVIDERS.iter()
        .find(|(n, _, _, _)| *n == app.setup_provider)
        .copied()
        .unwrap_or(PROVIDERS[0]);

    let inner = centered_box(area, 70, 8);
    let block = Block::default()
        .title(format!(" {label} API key "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let content_area = block.inner(inner);
    f.render_widget(block, inner);

    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled(note, Style::default().fg(Color::DarkGray))),
        Line::raw(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(app.input.clone()),
        ]),
    ];

    f.render_widget(Paragraph::new(Text::from(lines)), content_area);

    let cx = content_area.x + 2 + app.cursor_pos as u16;
    f.set_cursor_position((cx.min(content_area.x + content_area.width - 1), content_area.y + 3));
}

fn render_setup_model(f: &mut Frame, app: &App, area: Rect) {
    let (_, label, _, _) = PROVIDERS.iter()
        .find(|(n, _, _, _)| *n == app.setup_provider)
        .copied()
        .unwrap_or(PROVIDERS[0]);

    let inner = centered_box(area, 60, 7);
    let block = Block::default()
        .title(format!(" {label} — model "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let content_area = block.inner(inner);
    f.render_widget(block, inner);

    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled("edit or press enter to accept", Style::default().fg(Color::DarkGray))),
        Line::raw(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(app.input.clone()),
        ]),
    ];

    f.render_widget(Paragraph::new(Text::from(lines)), content_area);

    let cx = content_area.x + 2 + app.cursor_pos as u16;
    f.set_cursor_position((cx.min(content_area.x + content_area.width - 1), content_area.y + 3));
}

fn centered_box(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

fn render_chat(f: &mut Frame, app: &App, area: Rect) {
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

fn render_diff(f: &mut Frame, app: &App, area: Rect) {
    let (title, border_color) = match &app.status {
        Status::AwaitingPermission => (" Permission ", Color::Yellow),
        Status::Executing          => (" Running ",    Color::Cyan),
        _                          => (" Diff ",       Color::DarkGray),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = if app.status == Status::AwaitingPermission {
        render_permission_content(app)
    } else if app.diff_content.is_empty() {
        Text::from(vec![Line::from(Span::styled("no changes yet", Style::default().fg(Color::DarkGray)))])
    } else {
        Text::from(app.diff_content.lines().map(|l| {
            if l.starts_with("+ ")       { Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Green))) }
            else if l.starts_with("- ")  { Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Red))) }
            else if l.starts_with('@')   { Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Cyan))) }
            else if l.starts_with("tool:") { Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Yellow))) }
            else                         { Line::from(l.to_string()) }
        }).collect::<Vec<_>>())
    };

    f.render_widget(Paragraph::new(content).wrap(Wrap { trim: false }), inner);
}

fn render_permission_content(app: &App) -> Text<'static> {
    let Some(call) = app.pending_calls.get(app.current_call_idx) else { return Text::raw("") };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("tool  ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(call.name.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::raw(""),
    ];

    if let Some(obj) = call.input.as_object() {
        for (k, v) in obj {
            let owned = v.to_string();
            lines.push(Line::from(vec![
                Span::styled(format!("{k}  "), Style::default().fg(Color::DarkGray)),
                Span::raw(v.as_str().unwrap_or(&owned).to_string()),
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
}

fn render_session_list(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Sessions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.session_list.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("no saved sessions", Style::default().fg(Color::DarkGray))),
            inner,
        );
        return;
    }

    let lines: Vec<Line> = app.session_list.iter().enumerate().map(|(i, s)| {
        let selected = i == app.session_selected;
        let date = s.updated_at.format("%m/%d %H:%M").to_string();
        let max_title = inner.width.saturating_sub(12) as usize;
        let title = if s.title.len() > max_title { format!("{}…", &s.title[..max_title.saturating_sub(1)]) } else { s.title.clone() };

        if selected {
            Line::from(vec![
                Span::styled(format!("▶ {title}"), Style::default().fg(Color::Black).bg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {date}"), Style::default().fg(Color::Black).bg(Color::Blue)),
            ])
        } else {
            Line::from(vec![
                Span::styled(format!("  {title}"), Style::default().fg(Color::White)),
                Span::styled(format!("  {date}"), Style::default().fg(Color::DarkGray)),
            ])
        }
    }).collect();

    let scroll = (app.session_selected as u16).saturating_sub(inner.height.saturating_sub(1));
    f.render_widget(Paragraph::new(Text::from(lines)).scroll((scroll, 0)), inner);
}

fn render_session_preview(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = if let Some(meta) = app.session_list.get(app.session_selected) {
        if let Ok(s) = crate::session::load(&meta.id) {
            let lines: Vec<Line> = s.messages.iter().filter_map(|m| {
                let text = m.content.text();
                if text.is_empty() { return None; }
                let (label, color) = match m.role {
                    Role::User      => ("you", Color::Cyan),
                    Role::Assistant => ("ai ", Color::Green),
                    Role::System    => return None,
                };
                let snippet = if text.len() > 80 { format!("{}…", &text[..77]) } else { text.to_string() };
                Some(Line::from(vec![
                    Span::styled(format!("{label}  "), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                    Span::raw(snippet),
                ]))
            }).collect();
            Text::from(lines)
        } else {
            Text::from(Span::styled("failed to load", Style::default().fg(Color::Red)))
        }
    } else {
        Text::from(Span::styled("no sessions", Style::default().fg(Color::DarkGray)))
    };

    f.render_widget(Paragraph::new(content).wrap(Wrap { trim: false }), inner);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let total = app.tokens.input + app.tokens.output;

    let status_span = match &app.status {
        Status::Ready              => Span::styled("● ready",      Style::default().fg(Color::Green)),
        Status::Streaming          => Span::styled("◎ streaming",  Style::default().fg(Color::Yellow)),
        Status::AwaitingPermission => Span::styled("? permission", Style::default().fg(Color::Yellow)),
        Status::Executing          => Span::styled("⚙ executing",  Style::default().fg(Color::Cyan)),
        Status::SessionBrowser     => Span::styled("⎗ sessions",   Style::default().fg(Color::Blue)),
        Status::Setup(_)           => Span::styled("◌ setup",      Style::default().fg(Color::Blue)),
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

fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let (border_color, hint) = match &app.status {
        Status::Streaming | Status::Executing => (Color::Yellow, "ctrl+c to interrupt"),
        Status::AwaitingPermission            => (Color::Yellow, "y / n / a"),
        Status::SessionBrowser                => (Color::Blue,   "↑↓ navigate   enter load   esc cancel"),
        Status::Error(_)                      => (Color::Red,    ""),
        _                                     => (Color::Blue,   ""),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let blocked = matches!(
        app.status,
        Status::Streaming | Status::Executing | Status::AwaitingPermission | Status::SessionBrowser | Status::Setup(_)
    );

    let prompt = Span::styled("> ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD));
    let body = if blocked {
        Span::styled(hint, Style::default().fg(Color::DarkGray))
    } else {
        Span::raw(app.input.clone())
    };

    f.render_widget(Paragraph::new(Line::from(vec![prompt, body])), inner);

    if !blocked {
        let x = (inner.x + 2 + app.cursor_pos as u16).min(inner.x + inner.width.saturating_sub(1));
        f.set_cursor_position((x, inner.y));
    }
}

fn fmt_k(n: u32) -> String {
    if n >= 1_000 { format!("{:.1}k", n as f32 / 1_000.0) } else { n.to_string() }
}
