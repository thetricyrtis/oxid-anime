mod app;
mod ui;
mod api;
mod image_manager;
mod player;
mod search;
mod database;
mod models;

use ratatui::{backend::{Backend, CrosstermBackend}, Terminal};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, io, time::Duration};
use app::App;
use models::{InputMode, ViewMode, SearchMode};
use ui::draw_main_ui;
use search::{SearchManager, SearchResult};
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Ensure database directories exist and extract JSONL if necessary
    let _ = database::check_and_extract_db();

    // Initialize SQLite and populate if empty
    let mut db_conn = database::init_db().expect("Failed to initialize database");
    // Try to populate from existing JSONL (needs &mut for transaction)
    let _ = database::populate_db_from_jsonl(&mut db_conn);
    let db_conn = Arc::new(Mutex::new(db_conn));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(db_conn);
    app.load_language("en");
    app.load_db_version();
    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Critical Error: {:?}", err)
    }
    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    terminal.draw(|f| draw_main_ui(f, app))?;

    let search_manager = SearchManager::new(app.search_tx.clone());

    loop {
        let mut should_render = false;

        while let Ok((id, url, proto)) = app.image_manager.try_recv() {
            if id == app.current_search_id || id == 0 {
                app.image_manager.cache_insert(url, proto);
                app.update_current_image_from_cache();
                should_render = true;
            }
        }

        while let Ok(result) = app.db_update_rx.try_recv() {
            match result {
                models::DbUpdateResult::Progress(status_key) => {
                    if status_key.starts_with("db_updating_downloading:") {
                        let parts: Vec<&str> = status_key.split(':').collect();
                        let tag = parts.get(1).unwrap_or(&"");
                        let raw_prefix = app.t("db_updating_downloading");
                        app.db_update_status = Some(raw_prefix.replace("{version}", tag));
                    } else {
                        app.db_update_status = Some(app.t(&status_key).to_string());
                    }
                }
                models::DbUpdateResult::Success(version) => {
                    app.config.db_version = version;
                    app.db_update_status = None;
                    // No need to reload in-memory list, SQLite is already updated
                }
                models::DbUpdateResult::Error(err) => {
                    let err_prefix = app.t("label_error");
                    app.db_update_status = Some(format!("{}: {}", err_prefix, err));
                }
            }
            should_render = true;
        }

        while let Ok(result) = app.search_rx.try_recv() {
            match result {
                SearchResult::Success { id, results } => {
                    if id == app.current_search_id {
                        app.anime_list = results;
                        app.list_state.select(if app.anime_list.is_empty() { None } else { Some(0) });
                        app.update_current_image_from_cache();
                        app.error_message = None; 
                        
                        if !app.anime_list.is_empty() {
                            app.image_manager.preload_images(&app.anime_list, 3, app.current_search_id);
                            app.image_manager.request_image(&app.anime_list, app.list_state.selected());
                        }
                    }
                }
                SearchResult::Timeout => {
                    app.error_message = Some(app.t("err_timeout").to_string());
                }
                SearchResult::ApiError { id, message } => {
                    if id == app.current_search_id {
                        let prefix = app.t("err_api_prefix");
                        let formatted_err = prefix
                            .replace("{id}", &id.to_string())
                            .replace("{error}", &message);
                        app.error_message = Some(formatted_err);
                    }
                }
                SearchResult::Unknown => {
                    app.error_message = Some(app.t("err_unknown").to_string());
                }
            }
            should_render = true;
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match app.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('i') => {
                                if app.tab_index == 0 {
                                    app.input_mode = InputMode::Editing;
                                    should_render = true;
                                }
                            }
                            KeyCode::Char('h') | KeyCode::Left => {
                                if app.view_mode == ViewMode::List {
                                    app.tab_index = (app.tab_index + 2) % 3;
                                    should_render = true;
                                }
                            }
                            KeyCode::Char('l') | KeyCode::Right => {
                                if app.view_mode == ViewMode::List {
                                    app.tab_index = (app.tab_index + 1) % 3;
                                    should_render = true;
                                }
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                if app.tab_index == 0 && app.view_mode == ViewMode::List {
                                    app.next_anime();
                                    if let Some(selected) = app.list_state.selected() {
                                        if selected < app.anime_list.len() {
                                            app.image_manager.preload_images(&app.anime_list[selected..], 3, app.current_search_id);
                                            app.image_manager.request_image(&app.anime_list, Some(selected));
                                        }
                                    }
                                    should_render = true;
                                } else if app.tab_index == 0 && app.view_mode == ViewMode::Detail {
                                    app.handle_down_key();
                                    should_render = true;
                                } else if app.tab_index == 2 {
                                    app.next_setting();
                                    should_render = true;
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if app.tab_index == 0 && app.view_mode == ViewMode::List {
                                    app.previous_anime();
                                    if let Some(selected) = app.list_state.selected() {
                                        app.image_manager.preload_images(&app.anime_list[selected..], 3, app.current_search_id);
                                        app.image_manager.request_image(&app.anime_list, Some(selected));
                                    }
                                    should_render = true;
                                } else if app.tab_index == 0 && app.view_mode == ViewMode::Detail {
                                    app.handle_up_key();
                                    should_render = true;
                                } else if app.tab_index == 2 {
                                    app.previous_setting();
                                    should_render = true;
                                }
                            }
                            KeyCode::Enter => {
                                if app.tab_index == 0 {
                                    if app.view_mode == ViewMode::List && app.list_state.selected().is_some() {
                                        app.view_mode = ViewMode::Detail;
                                        app.episode_state.select(Some(0));
                                        should_render = true;
                                    } else if app.view_mode == ViewMode::Detail {
                                        app.play_selected_video();
                                    }
                                } else if app.tab_index == 2 {
                                    app.toggle_setting();
                                    should_render = true;
                                }
                            }
                            KeyCode::Backspace => {
                                if app.tab_index == 0 && app.view_mode == ViewMode::Detail {
                                    app.view_mode = ViewMode::List;
                                    should_render = true;
                                }
                            }
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Char(c) => {
                                app.search_input.push(c);
                                app.current_search_id += 1;
                                if app.config.search_mode != SearchMode::Online {
                                    let query = app.search_input.clone();
                                    search_manager.trigger_live_search(query, app.current_search_id, app.db_conn.clone());
                                }
                                should_render = true;
                            }
                            KeyCode::Backspace => {
                                app.search_input.pop();
                                if !app.search_input.is_empty() {
                                    app.current_search_id += 1;
                                    if app.config.search_mode != SearchMode::Online {
                                        let query = app.search_input.clone();
                                        search_manager.trigger_live_search(query, app.current_search_id, app.db_conn.clone());
                                    }
                                } else {
                                    app.anime_list.clear();
                                    app.list_state.select(None);
                                }
                                should_render = true;
                            }
                            KeyCode::Enter => {
                                app.input_mode = InputMode::Normal;
                                if !app.search_input.is_empty() {
                                    app.current_search_id += 1;
                                    search_manager.trigger_search_on_submit(
                                        app.search_input.clone(),
                                        app.current_search_id,
                                        app.config.search_mode,
                                        app.db_conn.clone(),
                                        app.anime_list.clone(),
                                    );
                                }
                                should_render = true;
                            }
                            KeyCode::Esc => {
                                app.input_mode = InputMode::Normal;
                                should_render = true;
                            }
                            _ => {}
                        },
                    }
                }
            }
        }

        if should_render {
            terminal.draw(|f| draw_main_ui(f, app))?;
        }
    }
}
