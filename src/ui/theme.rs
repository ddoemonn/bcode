use ratatui::style::{Color, Modifier, Style};

pub const BORDER: Color = Color::Rgb(55, 55, 65);
pub const BORDER_FOCUS: Color = Color::Rgb(90, 90, 200);
pub const BORDER_WARN: Color = Color::Rgb(200, 150, 0);
pub const BORDER_ERR: Color = Color::Rgb(200, 60, 60);
pub const BORDER_INFO: Color = Color::Rgb(60, 120, 180);

pub const USER: Color = Color::Rgb(110, 210, 255);
pub const AI: Color = Color::Rgb(120, 220, 130);
pub const TOOL: Color = Color::Rgb(240, 195, 80);
pub const ERROR: Color = Color::Rgb(240, 80, 80);
pub const DIM: Color = Color::Rgb(85, 85, 100);
pub const SUBTLE: Color = Color::Rgb(120, 120, 140);
pub const CODE: Color = Color::Rgb(195, 195, 240);
pub const CODE_LANG: Color = Color::Rgb(140, 140, 195);
pub const DIFF_ADD: Color = Color::Rgb(80, 210, 100);
pub const DIFF_REM: Color = Color::Rgb(225, 80, 80);
pub const DIFF_HUNK: Color = Color::Rgb(80, 160, 225);
pub const HEADER: Color = Color::Rgb(160, 180, 255);
pub const BULLET: Color = Color::Rgb(110, 210, 255);

pub const RISK_READ: Color = Color::Rgb(80, 200, 120);
pub const RISK_WRITE: Color = Color::Rgb(240, 195, 80);
pub const RISK_SHELL: Color = Color::Rgb(240, 80, 80);

pub fn label(color: Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(DIM)
}

pub fn subtle() -> Style {
    Style::default().fg(SUBTLE)
}

pub fn code() -> Style {
    Style::default().fg(CODE)
}

pub fn border(color: Color) -> Style {
    Style::default().fg(color)
}

pub fn tool_risk_color(tool_name: &str) -> Color {
    match tool_name {
        "read_file" | "list_dir" | "glob" | "search_in_files" => RISK_READ,
        "write_file" | "replace_in_file" => RISK_WRITE,
        "bash" => RISK_SHELL,
        _ => RISK_WRITE,
    }
}

pub fn tool_risk_label(tool_name: &str) -> &'static str {
    match tool_name {
        "read_file" | "list_dir" | "glob" | "search_in_files" => "read",
        "write_file" | "replace_in_file" => "write",
        "bash" => "shell",
        _ => "exec",
    }
}
