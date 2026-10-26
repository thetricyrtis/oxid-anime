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
pub mod library_tab; // NEW: Register the module

pub fn draw_main_ui(f: &mut Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    let titles = vec![
        app.t("tab_search"), 
        app.t("tab_library"), 
        app.t("tab_settings")
    ];
    
    let tabs = Tabs::new(titles)
        .block(components::create_block(app.t("title_main")))
        .select(app.tab_index)
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, main_layout[0]);

    // Render the active tab content
    match app.tab_index {
        0 => search_tab::draw(f, app, main_layout[1]),
        1 => library_tab::draw(f, app, main_layout[1]), // NEW: Render library tab
        2 => settings_tab::draw(f, app, main_layout[1]),
        _ => {}
    }

    // Determine footer text based on input mode and view focus
    let footer_text = match app.input_mode {
        InputMode::Normal => {
            // Special footer for Library tab
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
