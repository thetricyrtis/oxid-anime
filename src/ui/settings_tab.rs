use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{List, ListItem, Paragraph, Wrap},
    Frame,
};
use crate::app::App;
use super::components::create_block;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let search_mode_str = format!("{:?}", app.config.search_mode);

    let db_version_display = if let Some(status) = &app.db_update_status {
        status.clone()
    } else {
        app.config.db_version.clone()
    };

    let items = vec![
        ListItem::new(format!(" 1. {}: {}", app.t("label_lang"), app.config.language)),
        ListItem::new(format!(" 2. {}: {}", app.t("label_player"), app.config.player)),
        ListItem::new(format!(" 3. {}: {}", app.t("label_protocol"), app.config.image_protocol)),
        ListItem::new(format!(" 4. {}: {}", app.t("label_search_mode"), search_mode_str)),
        ListItem::new(format!(" 5. {}: {}", app.t("label_db_version"), db_version_display)),
        ListItem::new(format!(" 6. {}", app.t("label_clear_cache"))),
    ];
        
    let settings = List::new(items)
        .block(create_block(app.t("title_menu")))
        .highlight_style(Style::default().bg(Color::Rgb(40,40,40)).fg(Color::Yellow))
        .highlight_symbol(">> ");
    
    f.render_stateful_widget(settings, chunks[0], &mut app.settings_state);

    let detail_text = match app.settings_state.selected() {
        Some(0) => app.t("settings_desc_lang"),
        Some(1) => app.t("settings_desc_player"),
        Some(2) => app.t("settings_desc_protocol"),
        Some(3) => app.t("settings_desc_search_mode"),
        Some(4) => app.t("settings_desc_db_version"),
        Some(5) => app.t("settings_desc_clear_cache"),
        _ => "",
    };

    let detail_p = Paragraph::new(detail_text)
        .block(create_block(app.t("title_detail")))
        .wrap(Wrap { trim: false });
    f.render_widget(detail_p, chunks[1]);
}
