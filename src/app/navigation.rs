use crate::app::App;
use crate::models::{DetailFocus, SourceState};

impl App {
    pub fn next_anime(&mut self) {
        if self.anime_list.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i >= self.anime_list.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
        self.update_current_image_from_cache();
    }

    pub fn previous_anime(&mut self) {
        if self.anime_list.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { self.anime_list.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
        self.update_current_image_from_cache();
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

    // --- YENİ DETAY PANELİ & KAYNAK YÖNLENDİRMELERİ ---

    // Sol panele odaklan (h / Sol Yön Tuşu)
    pub fn focus_left_panel(&mut self) {
        self.detail_focus = DetailFocus::Episodes;
    }

    // Sağ panele odaklan (l / Sağ Yön Tuşu)
    pub fn focus_right_panel(&mut self) {
        if matches!(self.source_state, SourceState::SelectSource { .. }) {
            self.detail_focus = DetailFocus::Sources;
        }
    }

    // Aşağı yönlü hareket (j / Down Tuşu)
    pub fn handle_down_key(&mut self) {
        match self.detail_focus {
            DetailFocus::Episodes => self.next_episode(),
            DetailFocus::Sources => self.next_source(),
        }
    }

    // Yukarı yönlü hareket (k / Up Tuşu)
    pub fn handle_up_key(&mut self) {
        match self.detail_focus {
            DetailFocus::Episodes => self.previous_episode(),
            DetailFocus::Sources => self.previous_source(),
        }
    }

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

    // Bölüm üzerinde Enter'a basılınca Scraping başlatma
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

    // Sağ taraftaki kaynak seçilip Enter'a basılınca MPV/VLC başlatma
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
}
