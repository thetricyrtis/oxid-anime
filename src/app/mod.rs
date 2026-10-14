use ratatui::widgets::ListState;
use ratatui_image::protocol::Protocol;
use crate::models::{AnimeInfo, InputMode, ViewMode, Config, OfflineAnimeEntry, DbUpdateResult, DetailFocus, SourceState};
use crate::image_manager::ImageManager;
use crate::search::SearchResult;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod navigation;
pub mod settings;

pub struct App {
    pub tab_index: usize,
    pub input_mode: InputMode,
    pub view_mode: ViewMode,
    pub search_input: String,
    pub translations: HashMap<String, String>,
    pub anime_list: Vec<AnimeInfo>,
    pub list_state: ListState,
    pub episode_state: ListState,
    pub settings_state: ListState,
    pub current_image: Option<Protocol>,
    pub image_manager: ImageManager,
    pub current_search_id: u64,
    pub error_message: Option<String>,
    pub config: Config,
    pub search_tx: mpsc::UnboundedSender<SearchResult>,
    pub search_rx: mpsc::UnboundedReceiver<SearchResult>,
    pub offline_db: Arc<Vec<OfflineAnimeEntry>>,
    pub db_update_status: Option<String>,
    pub db_update_tx: mpsc::UnboundedSender<DbUpdateResult>,
    pub db_update_rx: mpsc::UnboundedReceiver<DbUpdateResult>,
    
    // YENİ: Panelli detay ekranı odak ve kaynak durumları
    pub detail_focus: DetailFocus,
    pub source_state: SourceState,
}

impl App {
    pub fn new() -> Self {
        let mut settings_state = ListState::default();
        settings_state.select(Some(0));
        let (search_tx, search_rx) = mpsc::unbounded_channel::<SearchResult>();
        let (db_update_tx, db_update_rx) = mpsc::unbounded_channel::<DbUpdateResult>();
        let offline_db = Arc::new(crate::database::load_offline_db().unwrap_or_default());

        App {
            tab_index: 0,
            input_mode: InputMode::Normal,
            view_mode: ViewMode::List,
            search_input: String::new(),
            anime_list: Vec::new(),
            list_state: ListState::default(),
            episode_state: ListState::default(),
            settings_state,
            translations: HashMap::new(),
            current_image: None,
            image_manager: ImageManager::new(),
            current_search_id: 0,
            error_message: None,
            config: Config::default(),
            search_tx,
            search_rx,
            offline_db,
            db_update_status: None,
            db_update_tx,
            db_update_rx,
            
            // YENİ: Odak sol taraftaki bölümlerde ve kaynak durumu boş başlar
            detail_focus: DetailFocus::default(),
            source_state: SourceState::None,
        }
    }

    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.translations.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    pub fn models_view_mode_is_list(&self, mode: ViewMode) -> bool {
        mode == ViewMode::List
    }

    pub fn reload_offline_db(&mut self) {
        if let Ok(db) = crate::database::load_offline_db() {
            self.offline_db = Arc::new(db);
        }
    }
}
