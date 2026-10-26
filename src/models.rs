use serde::{Deserialize, Serialize};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

// --- Existing Enums & Structs ---

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum SearchMode {
    Offline,
    Online,
    Hybrid,
}

impl Default for SearchMode {
    fn default() -> Self {
        SearchMode::Hybrid
    }
}

impl SearchMode {
    pub fn next(&self) -> Self {
        match self {
            SearchMode::Offline => SearchMode::Online,
            SearchMode::Online => SearchMode::Hybrid,
            SearchMode::Hybrid => SearchMode::Offline,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ViewMode {
    List,
    Detail,
}

// UPDATED: Added Library variant. Sources kept for future plugin system, but hidden from UI.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DetailFocus {
    Episodes,
    Library,
    Sources, // Hidden in UI for now
}

impl Default for DetailFocus {
    fn default() -> Self {
        DetailFocus::Episodes
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub language: String,
    pub player: String,
    pub image_protocol: String,
    pub search_mode: SearchMode,
    pub db_version: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            language: "English".to_string(),
            player: "MPV".to_string(),
            image_protocol: "Kitty/Ghostty".to_string(),
            search_mode: SearchMode::Hybrid,
            db_version: "Not Installed".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeInfo {
    pub id: u32,
    pub title: String,
    pub status: String,
    pub episodes: Option<u32>,
    pub description: String,
    pub cover_image_url: String,
    pub score: Option<u32>,
    pub season_year: Option<u32>,
    pub genres: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

// --- Library Models (NEW) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryStatus {
    Watching,
    Planning,
    Completed,
    Dropped,
    Paused,
    Repeating,
}

impl Default for LibraryStatus {
    fn default() -> Self {
        LibraryStatus::Planning
    }
}

impl LibraryStatus {
    /// Returns the next status in the cycle for UI navigation.
    pub fn next(&self) -> Self {
        match self {
            LibraryStatus::Watching => LibraryStatus::Planning,
            LibraryStatus::Planning => LibraryStatus::Completed,
            LibraryStatus::Completed => LibraryStatus::Dropped,
            LibraryStatus::Dropped => LibraryStatus::Paused,
            LibraryStatus::Paused => LibraryStatus::Repeating,
            LibraryStatus::Repeating => LibraryStatus::Watching,
        }
    }

    /// Returns the previous status in the cycle for UI navigation.
    pub fn previous(&self) -> Self {
        match self {
            LibraryStatus::Watching => LibraryStatus::Repeating,
            LibraryStatus::Planning => LibraryStatus::Watching,
            LibraryStatus::Completed => LibraryStatus::Planning,
            LibraryStatus::Dropped => LibraryStatus::Completed,
            LibraryStatus::Paused => LibraryStatus::Dropped,
            LibraryStatus::Repeating => LibraryStatus::Paused,
        }
    }

    /// Converts the enum to a string for SQLite storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            LibraryStatus::Watching => "Watching",
            LibraryStatus::Planning => "Planning",
            LibraryStatus::Completed => "Completed",
            LibraryStatus::Dropped => "Dropped",
            LibraryStatus::Paused => "Paused",
            LibraryStatus::Repeating => "Repeating",
        }
    }
}

// Implement ToSql for LibraryStatus to store it in SQLite as TEXT
impl ToSql for LibraryStatus {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

// Implement FromSql for LibraryStatus to read it from SQLite
impl FromSql for LibraryStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let s = value.as_str()?;
        match s {
            "Watching" => Ok(LibraryStatus::Watching),
            "Planning" => Ok(LibraryStatus::Planning),
            "Completed" => Ok(LibraryStatus::Completed),
            "Dropped" => Ok(LibraryStatus::Dropped),
            "Paused" => Ok(LibraryStatus::Paused),
            "Repeating" => Ok(LibraryStatus::Repeating),
            _ => Err(FromSqlError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid LibraryStatus",
            )))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LibraryEntry {
    pub anilist_id: u32,
    pub title: String,
    pub status: LibraryStatus,
    pub progress: u32, // Watched episodes
    pub score: Option<u32>,
    pub updated_at: i64, // Unix timestamp
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    None,
    DeleteFromLibrary { anime_id: u32 },
}

// --- API & Offline DB Models ---

#[derive(Deserialize)]
pub struct ApiResponse {
    pub data: Data,
}

#[derive(Deserialize)]
pub struct Data {
    #[serde(rename = "Page")]
    pub page: Page,
}

#[derive(Deserialize)]
pub struct Page {
    #[serde(default)]
    pub media: Vec<Media>,
}

#[derive(Deserialize)]
pub struct Media {
    pub id: Option<u32>,
    pub title: ApiTitle,
    pub episodes: Option<u32>,
    pub status: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "coverImage")]
    pub cover_image: CoverImage,
    #[serde(rename = "averageScore")]
    pub average_score: Option<u32>,
    #[serde(rename = "seasonYear")]
    pub season_year: Option<u32>,
    pub genres: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
pub struct ApiTitle {
    pub english: Option<String>,
    pub romaji: String,
}

#[derive(Deserialize)]
pub struct CoverImage {
    pub large: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OfflineAnimeSeason {
    pub season: String,
    pub year: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OfflineAnimeEntry {
    #[serde(default)]
    pub sources: Vec<String>,
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(rename = "type", default)]
    pub type_field: String,
    pub episodes: Option<u32>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(rename = "relatedAnime", default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(rename = "animeSeason")]
    pub anime_season: Option<OfflineAnimeSeason>,
}

impl OfflineAnimeEntry {
    pub fn anilist_id(&self) -> Option<u32> {
        for source in &self.sources {
            if source.contains("anilist.co/anime/") {
                if let Some(id_str) = source.trim_end_matches('/').split('/').last() {
                    if let Ok(parsed_id) = id_str.parse::<u32>() {
                        return Some(parsed_id);
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub enum DbUpdateResult {
    Progress(String),
    Success(String),
    Error(String),
}

// --- Plugin / Source Models (Kept for future, hidden from UI) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSource {
    pub name: String,
    pub url: String,
    pub quality: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SourceState {
    None,
    Scraping {
        episode_num: u32,
    },
    SelectSource {
        episode_num: u32,
        sources: Vec<StreamSource>,
        selected_index: usize,
    },
    Error(String),
}
