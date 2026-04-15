use crate::app::App;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span, Text},
};
use super::theme;

pub fn render_content(app: &App) -> Text<'static> {
    let Some(call) = app.pending_calls.get(app.current_call_idx) else {
        return Text::raw("");
    };

    let risk_color = theme::tool_risk_color(&call.name);
    let risk_label = theme::tool_risk_label(&call.name);

    let mut lines = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                call.name.clone(),
                Style::default().fg(ratatui::style::Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!(" {risk_label} "),
                Style::default()
                    .fg(ratatui::style::Color::Black)
                    .bg(risk_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
    ];

    if let Some(obj) = call.input.as_object() {
        for (k, v) in obj {
            let val = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
            let truncated = if val.len() > 60 {
                format!("{}…", &val[..57])
            } else {
                val
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {k:<12}", k = k), theme::dim()),
                Span::styled(truncated, theme::subtle()),
            ]));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "  ─────────────────────────────",
        theme::dim(),
    )));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("  [y]", theme::label(theme::RISK_READ)),
        Span::styled(" allow once   ", theme::dim()),
        Span::styled("[n]", theme::label(theme::RISK_SHELL)),
        Span::styled(" deny   ", theme::dim()),
        Span::styled("[a]", theme::label(theme::TOOL)),
        Span::styled(" always allow", theme::dim()),
    ]));

    Text::from(lines)
}
