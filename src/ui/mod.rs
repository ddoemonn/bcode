mod chat;
mod diff;
mod input;
mod permission;
mod sessions;
mod setup;
mod status_bar;
pub mod theme;

use crate::app::{App, Status};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    if let Status::Setup(_) = &app.status {
        setup::render(f, app, area);
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
        sessions::render_list(f, app, panes[0]);
        sessions::render_preview(f, app, panes[1]);
    } else {
        chat::render(f, app, panes[0]);
        diff::render(f, app, panes[1]);
    }

    status_bar::render(f, app, outer[1]);
    input::render(f, app, outer[2]);
}
