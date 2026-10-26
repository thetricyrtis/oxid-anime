use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{List, ListItem, Paragraph, Wrap},
    Frame,
};
use crate::app::App;
use crate::models::LibraryStatus;
use super::components::create_block;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    // Split the screen into Left (List) and Right (Poster + Info)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    // Split the right side vertically into Poster (Top) and Info (Bottom)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    // --- LEFT: Library List ---
    let list_title = format!(" {} ({}) ", app.t("title_local"), app.library_list.len());
    
    if app.library_list.is_empty() {
        let empty_msg = Paragraph::new(format!("\n\n  {}", app.t("msg_library_empty")))
            .style(Style::default().fg(Color::DarkGray))
            .block(create_block(&list_title))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(empty_msg, chunks[0]);
    } else {
        let items: Vec<ListItem> = app.library_list.iter().map(|entry| {
            // Map status to an icon for quick visual scanning
            let icon = match entry.status {
                LibraryStatus::Watching => "󰐊",
                LibraryStatus::Completed => "󰄲",
                LibraryStatus::Planning => "󰄱",
                LibraryStatus::Dropped => "󰅖",
                LibraryStatus::Paused => "󰏤",
                LibraryStatus::Repeating => "󰑮",
            };
            
            // Format: [Icon] Title
            ListItem::new(format!(" {} {}", icon, entry.title))
        }).collect();

        let list = List::new(items)
            .block(create_block(&list_title))
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 40, 40))
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, chunks[0], &mut app.library_list_state);
    }

    // --- RIGHT TOP: Poster ---
    let poster_block = create_block(app.t("title_poster"));
    let poster_area = right_chunks[0];
    let poster_inner = poster_block.inner(poster_area);
    f.render_widget(poster_block, poster_area);

    if let Some(img_proto) = &mut app.current_image {
        let image_widget = ratatui_image::Image::new(img_proto);
        f.render_widget(image_widget, poster_inner);
    }

    // --- RIGHT BOTTOM: Selected Anime Info ---
    let info_block = create_block(app.t("title_summary"));
    let info_inner = info_block.inner(right_chunks[1]);
    f.render_widget(info_block, right_chunks[1]);

    if let Some(idx) = app.library_list_state.selected() {
        if let Some(entry) = app.library_list.get(idx) {
            // Format the status text based on the enum
            let status_str = match entry.status {
                LibraryStatus::Watching => app.t("status_watching"),
                LibraryStatus::Planning => app.t("status_planning"),
                LibraryStatus::Completed => app.t("status_completed"),
                LibraryStatus::Dropped => app.t("status_dropped"),
                LibraryStatus::Paused => app.t("status_paused"),
                LibraryStatus::Repeating => app.t("status_repeating"),
            };

            let info_text = vec![
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::raw(format!("{}: ", app.t("info_title"))),
                    ratatui::text::Span::styled(&entry.title, Style::default().add_modifier(Modifier::BOLD)),
                ]),
                ratatui::text::Line::from(""),
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::raw(format!("{}: ", app.t("label_status"))),
                    ratatui::text::Span::styled(status_str, Style::default().fg(Color::Cyan)),
                ]),
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::raw(format!("{}: ", app.t("label_progress"))),
                    ratatui::text::Span::styled(
                        entry.progress.to_string(),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                ]),
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::raw(format!("{}: ", app.t("info_score"))),
                    ratatui::text::Span::styled(
                        entry.score.map(|s| s.to_string()).unwrap_or_else(|| "-".to_string()),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
            ];

            let info_p = Paragraph::new(info_text)
                .block(ratatui::widgets::Block::default().padding(ratatui::widgets::Padding::new(1, 1, 1, 1)))
                .wrap(Wrap { trim: true });
            f.render_widget(info_p, info_inner);
        }
    }
}
