use ratatui::widgets::ListState;
use ratatui_image::protocol::Protocol;
use crate::models::{
    AnimeInfo, InputMode, ViewMode, Config, DbUpdateResult, DetailFocus, SourceState,
    LibraryEntry, ConfirmAction
};
use crate::image_manager::ImageManager;
use crate::search::SearchResult;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
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
    
    // SQLite connection (thread-safe)
    pub db_conn: Arc<Mutex<Connection>>,
    
    pub db_update_status: Option<String>,
    pub db_update_tx: mpsc::UnboundedSender<DbUpdateResult>,
    pub db_update_rx: mpsc::UnboundedReceiver<DbUpdateResult>,
    
    // Detail screen focus and source states
    pub detail_focus: DetailFocus,
    pub source_state: SourceState,

    // --- Library specific fields ---
    pub current_library_entry: Option<LibraryEntry>,
    pub confirm_action: ConfirmAction,
    pub library_status_index: usize,
    pub library_progress: u32,

    // NEW: List of all library entries for the Library Tab
    pub library_list: Vec<LibraryEntry>,
    pub library_list_state: ListState,
}

impl App {
    pub fn new(db_conn: Arc<Mutex<Connection>>) -> Self {
        let mut settings_state = ListState::default();
        settings_state.select(Some(0));
        let (search_tx, search_rx) = mpsc::unbounded_channel::<SearchResult>();
        let (db_update_tx, db_update_rx) = mpsc::unbounded_channel::<DbUpdateResult>();

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
            db_conn,
            db_update_status: None,
            db_update_tx,
            db_update_rx,
            
            detail_focus: DetailFocus::default(),
            source_state: SourceState::None,

            current_library_entry: None,
            confirm_action: ConfirmAction::None,
            library_status_index: 0,
            library_progress: 0,

            library_list: Vec::new(),
            library_list_state: ListState::default(),
        }
    }

    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.translations.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    pub fn models_view_mode_is_list(&self, mode: ViewMode) -> bool {
        mode == ViewMode::List
    }

    /// Loads the library entry for the currently selected anime.
    /// If not found in DB, it sets up default values for adding.
    pub fn load_current_library_entry(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(anime) = self.anime_list.get(idx) {
                let conn = self.db_conn.lock().unwrap();
                if let Ok(Some(entry)) = crate::database::get_library_entry(&conn, anime.id) {
                    self.library_status_index = match entry.status {
                        crate::models::LibraryStatus::Watching => 0,
                        crate::models::LibraryStatus::Planning => 1,
                        crate::models::LibraryStatus::Completed => 2,
                        crate::models::LibraryStatus::Dropped => 3,
                        crate::models::LibraryStatus::Paused => 4,
                        crate::models::LibraryStatus::Repeating => 5,
                    };
                    self.library_progress = entry.progress;
                    self.current_library_entry = Some(entry);
                } else {
                    self.library_status_index = 1; // Planning
                    self.library_progress = 0;
                    self.current_library_entry = None;
                }
            }
        }
    }

    /// Fetches all entries from the database and loads them into the app state.
    /// This should be called whenever the user switches to the Library tab.
    pub fn refresh_library_list(&mut self) {
        let conn = self.db_conn.lock().unwrap();
        if let Ok(entries) = crate::database::get_all_library_entries(&conn) {
            self.library_list = entries;
            
            if self.library_list.is_empty() {
                self.library_list_state.select(None);
            } else {
                self.library_list_state.select(Some(0));
            }
        }
    }

    /// Handles navigation (j/k) within the Library Tab.
    pub fn next_library_item(&mut self) {
        if self.library_list.is_empty() { return; }
        let i = match self.library_list_state.selected() {
            Some(i) => if i >= self.library_list.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.library_list_state.select(Some(i));
    }

    pub fn previous_library_item(&mut self) {
        if self.library_list.is_empty() { return; }
        let i = match self.library_list_state.selected() {
            Some(i) => if i == 0 { self.library_list.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.library_list_state.select(Some(i));
    }

    /// Opens the selected library entry in the detail view.
    /// Fetches full anime data from the DB, replaces the current anime_list,
    /// and switches to the detail view.
   pub fn open_library_entry_in_detail(&mut self) {
        // Get the selected library entry
        let entry = match self.library_list_state.selected() {
            Some(idx) => match self.library_list.get(idx) {
                Some(e) => e.clone(),
                None => return,
            },
            None => return,
        };

        // Fetch the full anime info from the DB
        let conn = self.db_conn.lock().unwrap();
        if let Ok(Some(anime)) = crate::database::get_anime_by_id(&conn, entry.anilist_id) {
            // NEW: Clear the previous search list and reset state
            self.anime_list.clear();
            self.list_state.select(None);
            self.current_search_id += 1; // Invalidate any pending search results
            self.error_message = None;

            // Insert the single anime from the library
            self.anime_list = vec![anime];
            self.list_state.select(Some(0));
            self.episode_state.select(Some(0));
            
            // Release the lock before calling load_current_library_entry (it re-locks)
            drop(conn); 
            self.load_current_library_entry();
            
            // Switch to search tab (which owns the detail view) and enter detail mode
            self.tab_index = 0;
            self.view_mode = ViewMode::Detail;
            self.detail_focus = DetailFocus::Episodes;

            // Request the poster for this anime
            self.image_manager.request_image(&self.anime_list, Some(0));
        }
    }
}
