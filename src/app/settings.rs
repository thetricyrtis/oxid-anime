use crate::app::App;
use std::collections::HashMap;
use std::fs;

impl App {
    pub fn load_language(&mut self, lang: &str) {
        let path = format!("locales/{}.json", lang);
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(parsed) = serde_json::from_str::<HashMap<String, String>>(&content) {
                self.translations = parsed;
            }
        }
    }

    pub fn toggle_setting(&mut self) {
        if let Some(idx) = self.settings_state.selected() {
            match idx {
                0 => {
                    if self.config.language == "Türkçe" {
                        self.config.language = "English".to_string();
                        self.load_language("en");
                    } else {
                        self.config.language = "Türkçe".to_string();
                        self.load_language("tr");
                    }
                }
                1 => {
                    self.config.player = if self.config.player == "MPV" { "VLC".to_string() } else { "MPV".to_string() };
                }
                2 => {
                    self.config.image_protocol = if self.config.image_protocol == "Kitty/Ghostty" { "Sixel".to_string() } else { "Kitty/Ghostty".to_string() };
                }
                3 => {
                    self.config.search_mode = self.config.search_mode.next();
                }
                4 => {
                    if self.db_update_status.is_none() {
                        crate::database::spawn_db_update(self.db_update_tx.clone());
                    }
                }
                5 => {
                    self.image_manager.clear_cache();
                    self.current_image = None;
                }
                _ => {}
            }
        }
    }

    pub fn update_current_image_from_cache(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(anime) = self.anime_list.get(idx) {
                if let Some(proto) = self.image_manager.cache_get(&anime.cover_image_url) {
                    self.current_image = Some(proto.clone());
                } else {
                    self.current_image = None;
                }
            }
        }
    }

    pub fn load_db_version(&mut self) {
        let version_path = "data/database/db_version.txt";
        if let Ok(version) = fs::read_to_string(version_path) {
            self.config.db_version = version.trim().to_string();
        } else {
            self.config.db_version = self.t("db_status_not_installed").to_string();
        }
    }
}
