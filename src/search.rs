use crate::models::{AnimeInfo, SearchMode};
use crate::api;
use tokio::sync::mpsc;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

pub enum SearchResult {
    Success { id: u64, results: Vec<AnimeInfo> },
    Timeout,
    ApiError { id: u64, message: String },
    #[allow(dead_code)]
    Unknown,
}

pub struct SearchManager {
    tx: mpsc::UnboundedSender<SearchResult>,
}

impl SearchManager {
    pub fn new(tx: mpsc::UnboundedSender<SearchResult>) -> Self {
        SearchManager { tx }
    }

    /// 1. Live search while typing: queries SQLite directly for offline results.
    pub fn trigger_live_search(&self, query: String, search_id: u64, db_conn: Arc<Mutex<Connection>>) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let results = perform_offline_search(&query, &db_conn);
            let _ = tx.send(SearchResult::Success { id: search_id, results });
        });
    }

    /// 2. On Enter/ESC: Handles offline, online, or hybrid search based on mode.
    pub fn trigger_search_on_submit(
        &self,
        query: String,
        search_id: u64,
        mode: SearchMode,
        db_conn: Arc<Mutex<Connection>>,
        current_list: Vec<AnimeInfo>,
    ) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match mode {
                SearchMode::Offline => {
                    let results = perform_offline_search(&query, &db_conn);
                    let _ = tx.send(SearchResult::Success { id: search_id, results });
                }
                SearchMode::Online => {
                    let search_future = api::search_anime(&query);
                    match tokio::time::timeout(Duration::from_secs(6), search_future).await {
                        Ok(Ok(animeler)) => {
                            let _ = tx.send(SearchResult::Success { id: search_id, results: animeler });
                        }
                        Ok(Err(e)) => {
                            let _ = tx.send(SearchResult::ApiError { id: search_id, message: e.to_string() });
                        }
                        Err(_) => {
                            let _ = tx.send(SearchResult::Timeout);
                        }
                    }
                }
                SearchMode::Hybrid => {
                    // Start with offline results as the base list
                    let mut base_list = if current_list.is_empty() {
                        perform_offline_search(&query, &db_conn)
                    } else {
                        current_list
                    };

                    // Send local results immediately for UI responsiveness
                    let _ = tx.send(SearchResult::Success { id: search_id, results: base_list.clone() });

                    // Enrich with AniList API in the background
                    let search_future = api::search_anime(&query);
                    match tokio::time::timeout(Duration::from_secs(6), search_future).await {
                        Ok(Ok(online_animeler)) => {
                            for local_anime in base_list.iter_mut() {
                                let local_title_lower = local_anime.title.to_lowercase();

                                // Tier 1: Match by AniList ID
                                // Tier 2: Match by exact title
                                let matched_online = online_animeler.iter().find(|online| {
                                    if local_anime.id != 0 && online.id != 0 && local_anime.id == online.id {
                                        return true;
                                    }
                                    online.title.to_lowercase() == local_title_lower
                                });

                                if let Some(online) = matched_online {
                                    // Preserve local cover image, enrich other fields
                                    local_anime.description = online.description.clone();
                                    local_anime.score = online.score;
                                    if online.season_year.is_some() {
                                        local_anime.season_year = online.season_year;
                                    }
                                    if !online.genres.is_empty() {
                                        local_anime.genres = online.genres.clone();
                                    }
                                }
                            }
                            let _ = tx.send(SearchResult::Success { id: search_id, results: base_list });
                        }
                        Ok(Err(_)) | Err(_) => {
                            // On API failure, keep the offline results on screen
                        }
                    }
                }
            }
        });
    }
}

/// Performs a fuzzy search against the SQLite database.
/// It uses a LIKE query for pre-filtering, then scores results with SkimMatcherV2.
pub fn perform_offline_search(query: &str, db_conn: &Arc<Mutex<Connection>>) -> Vec<AnimeInfo> {
    if query.trim().is_empty() {
        return Vec::new();
    }

    let query_lower = query.to_lowercase();
    let matcher = SkimMatcherV2::default();
    let conn = db_conn.lock().unwrap();

    // Pre-filter using SQL LIKE to limit the result set. 
    // We fetch up to 200 rows to score in Rust.
    let mut stmt = match conn.prepare(
        "SELECT id, title, status, episodes, picture, type_field, season, year, synonyms, tags, sources 
         FROM anime 
         WHERE title LIKE ?1 OR synonyms LIKE ?1 
         LIMIT 200"
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let pattern = format!("%{}%", query);
    let rows = stmt.query_map([&pattern], |row| {
        let sources_json: String = row.get(10)?;
        let synonyms_json: String = row.get(8)?;
        let tags_json: String = row.get(9)?;
        
        let sources: Vec<String> = serde_json::from_str(&sources_json).unwrap_or_default();
        let synonyms: Vec<String> = serde_json::from_str(&synonyms_json).unwrap_or_default();
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        Ok((row.get::<_, i64>(0)? as u32, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<u32>>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?, row.get::<_, Option<u32>>(7)?,
            synonyms, tags, sources))
    }).ok();

    let mut scored_entries: Vec<(i64, AnimeInfo)> = Vec::new();

    if let Some(rows) = rows {
        for row in rows.flatten() {
            let (id, title, status, episodes, picture, type_field, _season, year, synonyms, tags, sources) = row;
            let title_lower = title.to_lowercase();
            
            let mut best_score = if title_lower.contains(&query_lower) {
                100
            } else {
                matcher.fuzzy_match(&title, query).unwrap_or(0)
            };

            for syn in &synonyms {
                let syn_lower = syn.to_lowercase();
                if syn_lower.contains(&query_lower) {
                    if 90 > best_score {
                        best_score = 90;
                    }
                } else if let Some(score) = matcher.fuzzy_match(syn, query) {
                    if score > best_score {
                        best_score = score;
                    }
                }
            }

            if best_score > 30 {
                let mut description = format!("Type: {}", type_field.unwrap_or_default());
                if !synonyms.is_empty() {
                    description.push_str(&format!("\nSynonyms: {}", synonyms.join(", ")));
                }
                if !tags.is_empty() {
                    description.push_str(&format!("\nTags: {}", tags.join(", ")));
                }

                let anime_info = AnimeInfo {
                    id,
                    title,
                    status: status.unwrap_or_default(),
                    episodes,
                    description,
                    cover_image_url: picture.unwrap_or_default(),
                    score: None,
                    season_year: year,
                    genres: tags,
                    sources,
                };

                scored_entries.push((best_score, anime_info));
            }
        }
    }

    // Sort by score descending and take the top 40
    scored_entries.sort_by(|a, b| b.0.cmp(&a.0));
    scored_entries.into_iter().take(40).map(|(_, anime)| anime).collect()
}
