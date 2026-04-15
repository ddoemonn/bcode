use crate::app::{App, SetupStep, Status, PROVIDERS};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::theme;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" bcode — first run setup ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme::border(theme::BORDER_FOCUS));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);

    let hint = match &app.status {
        Status::Setup(SetupStep::ChooseProvider) => "↑ ↓  or  1 / 2 / 3 / 4  to select — enter to confirm",
        Status::Setup(SetupStep::EnterApiKey) => "paste your API key and press enter",
        Status::Setup(SetupStep::EnterModel) => "edit model name or press enter to accept the default",
        _ => "",
    };

    f.render_widget(
        Paragraph::new(Span::styled(hint, theme::dim())).alignment(Alignment::Center),
        rows[0],
    );

    match &app.status {
        Status::Setup(SetupStep::ChooseProvider) => render_choose(f, app, rows[1]),
        Status::Setup(SetupStep::EnterApiKey) => render_api_key(f, app, rows[1]),
        Status::Setup(SetupStep::EnterModel) => render_model(f, app, rows[1]),
        _ => {}
    }
}

fn render_choose(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::raw(""),
        Line::from(Span::styled("  choose a provider", theme::subtle())),
        Line::raw(""),
    ];

    for (i, (_, label, model, note)) in PROVIDERS.iter().enumerate() {
        let selected = i == app.setup_selected;
        let prefix = if selected { "  ▶ " } else { "    " };
        let num_style = if selected {
            Style::default().fg(theme::USER).add_modifier(Modifier::BOLD)
        } else {
            theme::dim()
        };
        let label_style = if selected {
            Style::default()
                .fg(ratatui::style::Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            theme::subtle()
        };
        let model_style = if selected {
            Style::default().fg(theme::CODE)
        } else {
            theme::dim()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{prefix}[{}] ", i + 1), num_style),
            Span::styled(format!("{label:<14}"), label_style),
            Span::styled(format!("{model:<26}"), model_style),
            Span::styled(note.to_string(), theme::dim()),
        ]));
    }

    f.render_widget(Paragraph::new(Text::from(lines)), area);
}

fn render_api_key(f: &mut Frame, app: &App, area: Rect) {
    let (_, label, _, note) = PROVIDERS
        .iter()
        .find(|(n, _, _, _)| *n == app.setup_provider)
        .copied()
        .unwrap_or(PROVIDERS[0]);

    let inner = centered_box(area, 68, 8);
    let block = Block::default()
        .title(format!(" {label} — API key "))
        .borders(Borders::ALL)
        .border_style(theme::border(theme::BORDER_FOCUS));

    let content_area = block.inner(inner);
    f.render_widget(block, inner);

    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled(note, theme::dim())),
        Line::raw(""),
        Line::from(vec![
            Span::styled("> ", theme::label(theme::BORDER_FOCUS)),
            Span::raw(app.input.clone()),
        ]),
    ];

    f.render_widget(Paragraph::new(Text::from(lines)), content_area);

    let cx = content_area.x + 2 + app.cursor_pos as u16;
    f.set_cursor_position((cx.min(content_area.x + content_area.width - 1), content_area.y + 3));
}

fn render_model(f: &mut Frame, app: &App, area: Rect) {
    let (_, label, _, _) = PROVIDERS
        .iter()
        .find(|(n, _, _, _)| *n == app.setup_provider)
        .copied()
        .unwrap_or(PROVIDERS[0]);

    let inner = centered_box(area, 58, 7);
    let block = Block::default()
        .title(format!(" {label} — model "))
        .borders(Borders::ALL)
        .border_style(theme::border(theme::BORDER_FOCUS));

    let content_area = block.inner(inner);
    f.render_widget(block, inner);

    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled("edit or press enter to accept the default", theme::dim())),
        Line::raw(""),
        Line::from(vec![
            Span::styled("> ", theme::label(theme::BORDER_FOCUS)),
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
