use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{List, ListItem, Paragraph, Wrap, Padding},
    Frame,
};
use crate::app::App;
use crate::models::{InputMode, ViewMode, DetailFocus, SourceState};
use super::components::create_block;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    if app.view_mode == ViewMode::List {
        // ... (List görünümü olduğu gibi kalıyor) ...
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
                let genres_str = if anime.genres.is_empty() { "-".to_string() } else { anime.genres.join(", ") };
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
        // DETAY GÖRÜNÜMÜ (DETAIL VIEW)
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(area);

        // Sol Taraf: Üst (Künye/Özet), Alt (Bölümler)
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[0]);

        // Sağ Taraf: Üst (Poster %60), Alt (Video Kaynakları %40)
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[1]);

        if let Some(idx) = app.list_state.selected() {
            if let Some(anime) = app.anime_list.get(idx) {
                let genres_str = if anime.genres.is_empty() { "-".to_string() } else { anime.genres.join(", ") };
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

                // Bölüm Listesi
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
            }
        }

        // Sağ Üst: Poster
        let poster_block = create_block(app.t("title_poster"));
        let poster_area = right_chunks[0];
        let poster_inner = poster_block.inner(poster_area);

        f.render_widget(poster_block, poster_area);
        if let Some(img_proto) = &mut app.current_image {
            let image_widget = ratatui_image::Image::new(img_proto);
            f.render_widget(image_widget, poster_inner);
        }

        // Sağ Alt: Video Kaynakları & Kazıma Paneli
        let is_sources_focused = app.detail_focus == DetailFocus::Sources;
        let sources_border_color = if is_sources_focused { Color::Yellow } else { Color::Reset };

        match &app.source_state {
            SourceState::None => {
                let p = Paragraph::new(format!("\n  {}", app.t("info_select_ep_prompt")))
                    .style(Style::default().fg(Color::DarkGray))
                    .block(create_block(app.t("title_sources")).border_style(Style::default().fg(sources_border_color)));
                f.render_widget(p, right_chunks[1]);
            }
            SourceState::Scraping { episode_num } => {
                let msg_template = app.t("modal_scraping_msg");
                let formatted_msg = msg_template.replace("{}", &episode_num.to_string());

                let p = Paragraph::new(formatted_msg)
                    .style(Style::default().fg(Color::Yellow))
                    .block(create_block(app.t("modal_scraping_title")).border_style(Style::default().fg(sources_border_color)));
                f.render_widget(p, right_chunks[1]);
            }
            SourceState::SelectSource { episode_num, sources, selected_index } => {
                let items: Vec<ListItem> = sources
                    .iter()
                    .map(|src| ListItem::new(format!("  󰕼 {:<12} | 󰓅 {:<6}", src.name, src.quality)))
                    .collect();

                let title_template = app.t("modal_select_source_title");
                let title = title_template.replace("{}", &episode_num.to_string());

                let list = List::new(items)
                    .block(create_block(&title).border_style(Style::default().fg(sources_border_color)))
                    .highlight_style(
                        Style::default()
                            .bg(Color::Rgb(50, 50, 80))
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(if is_sources_focused { ">> " } else { "  " });

                let mut source_list_state = ratatui::widgets::ListState::default();
                source_list_state.select(Some(*selected_index));

                f.render_stateful_widget(list, right_chunks[1], &mut source_list_state);
            }
            SourceState::Error(err_msg) => {
                let p = Paragraph::new(format!("\n  󰅚 {}: {}", app.t("label_error"), err_msg))
                    .style(Style::default().fg(Color::Red))
                    .block(create_block(app.t("modal_error_title")).border_style(Style::default().fg(sources_border_color)));
                f.render_widget(p, right_chunks[1]);
            }
        }
    }
}
