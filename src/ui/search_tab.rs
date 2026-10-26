use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    widgets::{List, ListItem, Paragraph, Wrap, Padding, Clear},
    Frame,
    text::{Line, Span},
};
use crate::app::App;
use crate::models::{InputMode, ViewMode, DetailFocus, ConfirmAction, LibraryStatus};
use super::components::create_block;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    if app.view_mode == ViewMode::List {
        // --- LIST VIEW ---
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let input_style = match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        };
        
        let input = Paragraph::new(&*app.search_input)
            .style(input_style)
            .block(create_block(app.t("title_search_input")));
        f.render_widget(input, chunks[0]);

        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(35), Constraint::Percentage(30)])
            .split(chunks[1]);

        let items: Vec<ListItem> = app.anime_list.iter()
            .map(|anime| ListItem::new(anime.title.clone()))
            .collect();

        let list_title = if app.error_message.is_some() {
            format!("{} - {}", app.t("title_results"), app.t("label_error"))
        } else {
            app.t("title_results").to_string()
        };

        let list = List::new(items)
            .block(create_block(&list_title))
            .highlight_style(Style::default().bg(Color::Rgb(40,40,40)).fg(Color::Yellow))
            .highlight_symbol(">> ");
        f.render_stateful_widget(list, content_chunks[0], &mut app.list_state);

        if let Some(idx) = app.list_state.selected() {
            if let Some(anime) = app.anime_list.get(idx) {
                // Limit genres to a maximum of 4 to prevent UI overflow in list view
                let genres_str = if anime.genres.is_empty() {
                    "-".to_string()
                } else if anime.genres.len() > 4 {
                    format!("{}, ...", anime.genres.iter().take(4).cloned().collect::<Vec<_>>().join(", "))
                } else {
                    anime.genres.join(", ")
                };
                let score_str = anime.score.map(|s| format!("{:.1}/10", s as f32 / 10.0)).unwrap_or_else(|| "??".to_string());
                let year_str = anime.season_year.map(|y| y.to_string()).unwrap_or_else(|| "??".to_string());

                let info_text = format!(
                    "\n {}{}\n\n 󰃭 {}{}\n 󰓎 {}{}\n 󰏆 {}{}\n\n 󰄱 {}{}\n 󰐊 {}{}\n\n\n {}",
                    app.t("info_title"), anime.title,
                    " ", year_str,
                    " ", score_str,     
                    " ", genres_str,   
                    " ", anime.status,
                    " ", anime.episodes.map(|e| e.to_string()).unwrap_or_else(|| "??".to_string()),
                    app.t("info_go_detail")
                );
                let info_p = Paragraph::new(info_text).block(create_block(app.t("title_summary"))).wrap(Wrap { trim: true });
                f.render_widget(info_p, content_chunks[1]);
            }
        } else if let Some(err) = &app.error_message {
            let err_p = Paragraph::new(format!("\n {}", err))
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .block(create_block(app.t("label_error")));
            f.render_widget(err_p, content_chunks[1]);
        } else {
            f.render_widget(create_block(app.t("title_summary")), content_chunks[1]);
        }

        let poster_block = create_block(app.t("title_poster"));
        let poster_area = content_chunks[2];
        let poster_inner = poster_block.inner(poster_area);
        
        f.render_widget(poster_block, poster_area);
        if let Some(img_proto) = &mut app.current_image {
            let image_widget = ratatui_image::Image::new(img_proto);
            f.render_widget(image_widget, poster_inner);
        }
    } else {
        // --- DETAIL VIEW ---
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(area);

        // Left Side: Top (Info/Summary), Bottom (Episodes)
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[0]);

        // Right Side: Top (Poster 60%), Bottom (Library 40%)
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[1]);

        if let Some(idx) = app.list_state.selected() {
            if let Some(anime) = app.anime_list.get(idx) {
                // --- LEFT TOP: Info / Summary ---
                // Limit genres to a maximum of 4 to prevent UI overflow
                let genres_str = if anime.genres.is_empty() {
                    "-".to_string()
                } else if anime.genres.len() > 4 {
                    format!("{}, ...", anime.genres.iter().take(4).cloned().collect::<Vec<_>>().join(", "))
                } else {
                    anime.genres.join(", ")
                };
                let score_str = anime.score.map(|s| format!("{:.1}/10", s as f32 / 10.0)).unwrap_or_else(|| "??".to_string());
                let year_str = anime.season_year.map(|y| y.to_string()).unwrap_or_else(|| "??".to_string());
                let desc = if anime.description.is_empty() { app.t("info_no_description") } else { &anime.description };

                let full_detail_text = format!(
                    "󰄱 [{}] {}  |  󰓎 {}  |  󰃭 {}\n\n󰏆 {} {}\n󰐊 {} {}\n\n--------------------------------------------------\n\n{}",
                    anime.status, anime.title, score_str, year_str,
                    app.t("info_genres"), genres_str,
                    app.t("info_episodes"), anime.episodes.map(|e| e.to_string()).unwrap_or_else(|| "??".to_string()),
                    desc
                );

                let desc_p = Paragraph::new(full_detail_text)
                    .block(create_block(app.t("label_summary")).padding(Padding::new(1,1,1,1)))
                    .wrap(Wrap { trim: false });
                f.render_widget(desc_p, left_chunks[0]);

                // --- LEFT BOTTOM: Episodes List ---
                let ep_count = anime.episodes.unwrap_or(12);
                let ep_items: Vec<ListItem> = (1..=ep_count)
                    .map(|i| ListItem::new(format!("  {} {}", app.t("label_episode"), i)))
                    .collect();

                let is_ep_focused = app.detail_focus == DetailFocus::Episodes;
                let ep_border_color = if is_ep_focused { Color::Yellow } else { Color::Reset };

                let ep_list = List::new(ep_items)
                    .block(create_block(app.t("title_episodes")).border_style(Style::default().fg(ep_border_color)))
                    .highlight_style(Style::default().bg(Color::Rgb(40,40,40)).fg(Color::Cyan))
                    .highlight_symbol(if is_ep_focused { ">> " } else { "  " });
                f.render_stateful_widget(ep_list, left_chunks[1], &mut app.episode_state);

                // --- RIGHT TOP: Poster ---
                let poster_block = create_block(app.t("title_poster"));
                let poster_area = right_chunks[0];
                let poster_inner = poster_block.inner(poster_area);
                f.render_widget(poster_block, poster_area);
                if let Some(img_proto) = &mut app.current_image {
                    let image_widget = ratatui_image::Image::new(img_proto);
                    f.render_widget(image_widget, poster_inner);
                }

                // --- RIGHT BOTTOM: Library Panel ---
                let is_lib_focused = app.detail_focus == DetailFocus::Library;
                let lib_border_color = if is_lib_focused { Color::Yellow } else { Color::Reset };
                
                let lib_block = create_block(app.t("title_library"))
                    .border_style(Style::default().fg(lib_border_color));

                // If anime is not in library yet
                if app.current_library_entry.is_none() {
                    let msg = Paragraph::new(format!("\n  {}\n\n  {}", app.t("msg_add_to_library"), app.t("info_episodes")))
                        .block(lib_block)
                        .style(Style::default().fg(Color::DarkGray));
                    f.render_widget(msg, right_chunks[1]);
                } else {
                    // Anime is in library, show status and progress
                    let statuses = vec![
                        LibraryStatus::Watching,
                        LibraryStatus::Planning,
                        LibraryStatus::Completed,
                        LibraryStatus::Dropped,
                        LibraryStatus::Paused,
                        LibraryStatus::Repeating,
                    ];

                    let mut items = Vec::new();
                    items.push(Line::from(Span::raw(format!("  {} ", app.t("label_status")))));

                    for (i, status) in statuses.iter().enumerate() {
                        let is_selected = i == app.library_status_index;
                        let prefix = if is_selected && is_lib_focused { ">> " } else { "   " };
                        let style = if is_selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() };
                        
                        let status_str = match status {
                            LibraryStatus::Watching => app.t("status_watching"),
                            LibraryStatus::Planning => app.t("status_planning"),
                            LibraryStatus::Completed => app.t("status_completed"),
                            LibraryStatus::Dropped => app.t("status_dropped"),
                            LibraryStatus::Paused => app.t("status_paused"),
                            LibraryStatus::Repeating => app.t("status_repeating"),
                        };
                        
                        items.push(Line::from(vec![
                            Span::styled(format!("{}{}", prefix, status_str), style)
                        ]));
                    }

                    items.push(Line::from(""));
                    items.push(Line::from(vec![
                        Span::raw(format!("  {} ", app.t("label_progress"))),
                        Span::styled(format!("{}", app.library_progress), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(format!(" / {}", anime.episodes.map(|e| e.to_string()).unwrap_or_else(|| "?".to_string()))),
                    ]));
                    
                    items.push(Line::from(Span::styled(
                        format!("  {}", app.t("msg_change_progress")),
                        Style::default().fg(Color::DarkGray)
                    )));

                    let lib_p = Paragraph::new(items).block(lib_block);
                    f.render_widget(lib_p, right_chunks[1]);
                }

                // --- MODAL: Delete Confirmation Popup ---
                if let ConfirmAction::DeleteFromLibrary { .. } = app.confirm_action {
                    let area = centered_rect(50, 20, f.area());
                    f.render_widget(Clear, area);
                    
                    let popup_block = create_block(app.t("title_confirm_delete"))
                        .border_style(Style::default().fg(Color::Red));
                    
                    // Use short message to prevent overflow
                    let text = Paragraph::new(format!(
                        "\n  {}\n\n  [y] {}   [n] {}",
                        app.t("msg_confirm_delete_short"),
                        app.t("msg_yes"),
                        app.t("msg_no")
                    ))
                    .block(popup_block)
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true })
                    .style(Style::default().fg(Color::White));
                    
                    f.render_widget(text, area);
                }
            }
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
