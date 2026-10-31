use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Paragraph, Tabs},
    Frame,
};
use crate::app::App;
use crate::models::{InputMode, ViewMode, DetailFocus};

pub mod components;
pub mod search_tab;
pub mod settings_tab;
pub mod library_tab;

pub fn draw_main_ui(f: &mut Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    // --- TOP BAR: fixed outer block, tabs on the left, status on the right ---
    let top_block = components::create_block(app.t("title_main"));
    let top_inner = top_block.inner(main_layout[0]);
    f.render_widget(top_block, main_layout[0]);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(40),
        ])
        .split(top_inner);

    let titles = vec![
        app.t("tab_search"),
        app.t("tab_library"),
        app.t("tab_settings"),
    ];

    let tabs = Tabs::new(titles)
        .select(app.tab_index)
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, top_chunks[0]);

    if let Some(status) = &app.db_update_status {
        let status_p = Paragraph::new(status.as_str())
            .style(Style::default().fg(Color::Yellow))
            .alignment(ratatui::layout::Alignment::Right);
        f.render_widget(status_p, top_chunks[1]);
    }

    // --- ACTIVE TAB CONTENT ---
    match app.tab_index {
        0 => search_tab::draw(f, app, main_layout[1]),
        1 => library_tab::draw(f, app, main_layout[1]),
        2 => settings_tab::draw(f, app, main_layout[1]),
        _ => {}
    }

    // --- FOOTER ---
    let footer_text = match app.input_mode {
        InputMode::Normal => {
            if app.tab_index == 1 {
                app.t("footer_library")
            } else if app.view_mode == ViewMode::List {
                app.t("footer_normal")
            } else {
                match app.detail_focus {
                    DetailFocus::Episodes => app.t("footer_detail"),
                    DetailFocus::Library => app.t("footer_detail_library"),
                    DetailFocus::Sources => app.t("footer_detail"),
                }
            }
        }
        InputMode::Editing => app.t("footer_editing"),
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().bg(Color::Rgb(30, 30, 30)).fg(Color::Gray));
    f.render_widget(footer, main_layout[2]);
}
