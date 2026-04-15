use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use crate::ui::theme;

pub fn to_lines(text: &str) -> Vec<Line<'static>> {
    let mut result: Vec<Line<'static>> = Vec::new();
    let mut in_code = false;

    for raw in text.lines() {
        if raw.starts_with("```") {
            if in_code {
                in_code = false;
                result.push(Line::raw(""));
            } else {
                let lang = raw[3..].trim().to_string();
                in_code = true;
                if !lang.is_empty() {
                    result.push(Line::from(Span::styled(
                        format!(" {lang} "),
                        Style::default().fg(theme::CODE_LANG),
                    )));
                }
            }
            continue;
        }

        if in_code {
            result.push(Line::from(Span::styled(
                raw.to_string(),
                Style::default().fg(theme::CODE),
            )));
            continue;
        }

        if raw.is_empty() {
            result.push(Line::raw(""));
            continue;
        }

        if let Some(rest) = raw.strip_prefix("### ") {
            result.push(Line::from(Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme::HEADER)
                    .add_modifier(Modifier::BOLD),
            )));
        } else if let Some(rest) = raw.strip_prefix("## ") {
            result.push(Line::from(Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme::HEADER)
                    .add_modifier(Modifier::BOLD),
            )));
        } else if let Some(rest) = raw.strip_prefix("# ") {
            result.push(Line::from(Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(ratatui::style::Color::White)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
        } else if raw.starts_with("- ")
            || raw.starts_with("* ")
            || raw.starts_with("+ ")
        {
            let rest = &raw[2..];
            let mut spans = vec![Span::styled("• ", Style::default().fg(theme::BULLET))];
            spans.extend(inline(rest));
            result.push(Line::from(spans));
        } else if raw.len() > 2
            && raw.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
        {
            if let Some(dot) = raw.find(". ") {
                let num = raw[..dot].to_string();
                let rest = &raw[dot + 2..];
                let mut spans =
                    vec![Span::styled(format!("{num}. "), Style::default().fg(theme::BULLET))];
                spans.extend(inline(rest));
                result.push(Line::from(spans));
            } else {
                result.push(Line::from(inline(raw)));
            }
        } else {
            result.push(Line::from(inline(raw)));
        }
    }

    result
}

fn inline(s: &str) -> Vec<Span<'static>> {
    let chars: Vec<char> = s.chars().collect();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut i = 0;
    let mut plain = String::new();

    while i < chars.len() {
        if chars[i] == '`' {
            if !plain.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut plain)));
            }
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '`' {
                i += 1;
            }
            let code: String = chars[start..i].iter().collect();
            spans.push(Span::styled(code, Style::default().fg(theme::CODE)));
            if i < chars.len() { i += 1; }
        } else if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if !plain.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut plain)));
            }
            i += 2;
            let start = i;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '*') {
                i += 1;
            }
            let bold: String = chars[start..i].iter().collect();
            spans.push(Span::styled(bold, Style::default().add_modifier(Modifier::BOLD)));
            if i + 1 < chars.len() { i += 2; } else { i = chars.len(); }
        } else if chars[i] == '*' {
            if !plain.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut plain)));
            }
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '*' {
                i += 1;
            }
            let italic: String = chars[start..i].iter().collect();
            spans.push(Span::styled(italic, Style::default().add_modifier(Modifier::ITALIC)));
            if i < chars.len() { i += 1; }
        } else {
            plain.push(chars[i]);
            i += 1;
        }
    }

    if !plain.is_empty() {
        spans.push(Span::raw(plain));
    }

    spans
}
