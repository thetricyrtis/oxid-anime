use crate::app::App;
use crate::models::{DetailFocus, SourceState, LibraryEntry, LibraryStatus, ConfirmAction};

impl App {
    // --- Existing Anime & Episode Navigation ---
    pub fn next_anime(&mut self) {
        if self.anime_list.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i >= self.anime_list.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
        self.update_current_image_from_cache();
        self.load_current_library_entry(); // Load library state for new anime
    }

    pub fn previous_anime(&mut self) {
        if self.anime_list.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { self.anime_list.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
        self.update_current_image_from_cache();
        self.load_current_library_entry(); // Load library state for new anime
    }

    pub fn next_episode(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(anime) = self.anime_list.get(idx) {
                let max_episodes = anime.episodes.unwrap_or(12) as usize;
                let i = match self.episode_state.selected() {
                    Some(i) => if i >= max_episodes - 1 { 0 } else { i + 1 },
                    None => 0,
                };
                self.episode_state.select(Some(i));
            }
        }
    }

    pub fn previous_episode(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(anime) = self.anime_list.get(idx) {
                let max_episodes = anime.episodes.unwrap_or(12) as usize;
                let i = match self.episode_state.selected() {
                    Some(i) => if i == 0 { max_episodes - 1 } else { i - 1 },
                    None => 0,
                };
                self.episode_state.select(Some(i));
            }
        }
    }

    // --- Settings Navigation ---
    pub fn next_setting(&mut self) {
        let i = match self.settings_state.selected() {
            Some(i) => if i >= 5 { 0 } else { i + 1 },
            None => 0,
        };
        self.settings_state.select(Some(i));
    }
    
    pub fn previous_setting(&mut self) {
        let i = match self.settings_state.selected() {
            Some(i) => if i == 0 { 5 } else { i - 1 },
            None => 0,
        };
        self.settings_state.select(Some(i));
    }

    // --- Detail View Focus (Left/Right switching) ---
    // UPDATED: Cycles only between Episodes and Library.
    pub fn focus_left_panel(&mut self) {
        self.detail_focus = DetailFocus::Episodes;
    }

    pub fn focus_right_panel(&mut self) {
        self.detail_focus = DetailFocus::Library;
    }

    // --- Up/Down Key Handling based on Focus ---
    pub fn handle_down_key(&mut self) {
        match self.detail_focus {
            DetailFocus::Episodes => self.next_episode(),
            DetailFocus::Library => self.next_library_status(),
            DetailFocus::Sources => self.next_source(), // Kept for plugin later
        }
    }

    pub fn handle_up_key(&mut self) {
        match self.detail_focus {
            DetailFocus::Episodes => self.previous_episode(),
            DetailFocus::Library => self.previous_library_status(),
            DetailFocus::Sources => self.previous_source(), // Kept for plugin later
        }
    }

    // --- Library Specific Actions ---
    
    /// Cycles through LibraryStatus enum
    pub fn next_library_status(&mut self) {
        let next_status = match self.library_status_index {
            0 => 1, // Watching -> Planning
            1 => 2, // Planning -> Completed
            2 => 3, // Completed -> Dropped
            3 => 4, // Dropped -> Paused
            4 => 5, // Paused -> Repeating
            _ => 0, // Repeating -> Watching
        };
        self.library_status_index = next_status;
    }

    pub fn previous_library_status(&mut self) {
        let prev_status = match self.library_status_index {
            0 => 5, // Watching -> Repeating
            1 => 0, // Planning -> Watching
            2 => 1, // Completed -> Planning
            3 => 2, // Dropped -> Completed
            4 => 3, // Paused -> Dropped
            _ => 4, // Repeating -> Paused
        };
        self.library_status_index = prev_status;
    }

    /// Increases progress (+ key)
    pub fn increase_library_progress(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(anime) = self.anime_list.get(idx) {
                let max_eps = anime.episodes.unwrap_or(9999);
                if self.library_progress < max_eps {
                    self.library_progress += 1;
                }
            }
        }
    }

    /// Decreases progress (- key)
    pub fn decrease_library_progress(&mut self) {
        if self.library_progress > 0 {
            self.library_progress -= 1;
        }
    }

    /// Saves the current library state to SQLite
    pub fn save_library_entry(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(anime) = self.anime_list.get(idx) {
                let status = match self.library_status_index {
                    0 => LibraryStatus::Watching,
                    1 => LibraryStatus::Planning,
                    2 => LibraryStatus::Completed,
                    3 => LibraryStatus::Dropped,
                    4 => LibraryStatus::Paused,
                    _ => LibraryStatus::Repeating,
                };

                let entry = LibraryEntry {
                    anilist_id: anime.id,
                    title: anime.title.clone(),
                    status,
                    progress: self.library_progress,
                    score: None, // Can be expanded later
                    updated_at: 0, // Handled by DB function
                };

                let conn = self.db_conn.lock().unwrap();
                if let Ok(_) = crate::database::add_or_update_library_entry(&conn, &entry) {
                    // Update local state so UI reflects the change immediately
                    self.current_library_entry = Some(entry);
                }
            }
        }
    }

    /// Triggers the delete confirmation popup
    pub fn request_delete_from_library(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(anime) = self.anime_list.get(idx) {
                self.confirm_action = ConfirmAction::DeleteFromLibrary { anime_id: anime.id };
            }
        }
    }

    /// Executes the confirmed action (e.g., deletion)
    pub fn execute_confirm_action(&mut self) {
        match self.confirm_action {
            ConfirmAction::DeleteFromLibrary { anime_id } => {
                let conn = self.db_conn.lock().unwrap();
                let _ = crate::database::remove_library_entry(&conn, anime_id);
                
                // Reset local state
                self.current_library_entry = None;
                self.library_progress = 0;
                self.library_status_index = 1; // Default to Planning
            }
            ConfirmAction::None => {}
        }
        self.confirm_action = ConfirmAction::None;
    }

    // --- Source Navigation (Kept for future plugin integration) ---
    pub fn next_source(&mut self) {
        if let SourceState::SelectSource { ref sources, ref mut selected_index, .. } = self.source_state {
            if !sources.is_empty() {
                if *selected_index >= sources.len() - 1 {
                    *selected_index = 0;
                } else {
                    *selected_index += 1;
                }
            }
        }
    }
    
    pub fn previous_source(&mut self) {
        if let SourceState::SelectSource { ref sources, ref mut selected_index, .. } = self.source_state {
            if !sources.is_empty() {
                if *selected_index == 0 {
                    *selected_index = sources.len() - 1;
                } else {
                    *selected_index -= 1;
                }
            }
        }
    }

    // --- Video Playback (Temporary before plugin system) ---
    pub fn play_selected_video(&mut self) {
        let anime_idx = match self.list_state.selected() {
            Some(idx) => idx,
            None => return,
        };

        let ep_idx = match self.episode_state.selected() {
            Some(idx) => idx,
            None => return,
        };

        if let Some(_anime) = self.anime_list.get(anime_idx) {
            let episode_num = (ep_idx + 1) as u32;
            self.source_state = SourceState::Scraping { episode_num };
        }
    }

    #[allow(dead_code)]
    pub fn confirm_source_selection(&mut self) {
        if let SourceState::SelectSource { ref sources, selected_index, .. } = self.source_state {
            if let Some(source) = sources.get(selected_index) {
                let stream_url = source.url.clone();
                let player_bin = if self.config.player.is_empty() {
                    "mpv".to_string()
                } else {
                    self.config.player.clone()
                };

                let _ = std::process::Command::new(&player_bin)
                    .arg(&stream_url)
                    .spawn();
            }
        }
    }
} // <-- BURASI EN SONDA OLMALI (impl App kapanışı)
