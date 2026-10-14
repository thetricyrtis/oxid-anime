use crate::models::{AnimeInfo, SearchMode, OfflineAnimeEntry};
use crate::api;
use tokio::sync::mpsc;
use std::time::Duration;
use std::sync::Arc;
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

    /// 1. Harf harf yazarken (Editing modu): Sadece offline database kullanır (API sıfır)
    pub fn trigger_live_search(&self, query: String, search_id: u64, offline_db: Arc<Vec<OfflineAnimeEntry>>) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let results = perform_offline_search(&query, &offline_db);
            let _ = tx.send(SearchResult::Success { id: search_id, results });
        });
    }

    /// 2. Enter/ESC basıldığında: Seçili arama moduna göre zenginleştirme veya API araması yapar
    pub fn trigger_search_on_submit(
        &self,
        query: String,
        search_id: u64,
        mode: SearchMode,
        offline_db: Arc<Vec<OfflineAnimeEntry>>,
        current_list: Vec<AnimeInfo>,
    ) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match mode {
                SearchMode::Offline => {
                    let results = perform_offline_search(&query, &offline_db);
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
                    // Taban liste: Eğer ekranda harf harf aramadan gelen sonuç varsa onu kullan, yoksa offline DB'den çek
                    let mut base_list = if current_list.is_empty() {
                        perform_offline_search(&query, &offline_db)
                    } else {
                        current_list
                    };

                    // Yerel aramayı anında ekrana bas
                    let _ = tx.send(SearchResult::Success { id: search_id, results: base_list.clone() });

                    // Arka planda AniList API sorgusu atıp zenginleştirme (enrichment) yap
                    let search_future = api::search_anime(&query);
                    match tokio::time::timeout(Duration::from_secs(6), search_future).await {
                        Ok(Ok(online_animeler)) => {
                            for local_anime in base_list.iter_mut() {
                                let local_title_lower = local_anime.title.to_lowercase();

                                // KATMANLI EŞLEŞTİRME MOTORU (Tier 1: ID -> Tier 2: Title Match)
                                let matched_online = online_animeler.iter().find(|online| {
                                    // Tier 1: AniList ID Eşleşmesi
                                    if local_anime.id != 0 && online.id != 0 && local_anime.id == online.id {
                                        return true;
                                    }
                                    // Tier 2: Birebir Başlık Eşleşmesi
                                    online.title.to_lowercase() == local_title_lower
                                });

                                if let Some(online) = matched_online {
                                    // KRİTİK: local_anime.cover_image_url EZİLMİYOR! (Local DB resmi kalır)
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
                            // Zenginleştirilmiş listeyi ekrana gönder
                            let _ = tx.send(SearchResult::Success { id: search_id, results: base_list });
                        }
                        Ok(Err(_)) | Err(_) => {
                            // API patlarsa yerel sonuçlar ekranda kalmaya devam eder
                        }
                    }
                }
            }
        });
    }
}

pub fn perform_offline_search(query: &str, db: &[OfflineAnimeEntry]) -> Vec<AnimeInfo> {
    if query.trim().is_empty() {
        return Vec::new();
    }

    let query_lower = query.to_lowercase();
    let matcher = SkimMatcherV2::default();

    let mut scored_entries: Vec<(i64, &OfflineAnimeEntry)> = db.iter()
        .filter_map(|entry| {
            let title_lower = entry.title.to_lowercase();
            let mut best_score = if title_lower.contains(&query_lower) {
                100
            } else {
                matcher.fuzzy_match(&entry.title, query).unwrap_or(0)
            };

            if let Some(syns) = &entry.synonyms {
                for syn in syns {
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
            }

            if best_score > 30 {
                Some((best_score, entry))
            } else {
                None
            }
        })
        .collect();

    scored_entries.sort_by(|a, b| b.0.cmp(&a.0));

    scored_entries.into_iter()
        .take(40)
        .map(|(_, entry)| {
            // models.rs içinde yazdığımız anilist_id() metodunu güvenle çağırıyoruz
            let id = entry.anilist_id().unwrap_or(0);
            let title = entry.title.clone();
            let status = entry.status.clone();
            let episodes = entry.episodes;
            let cover_image_url = entry.picture.clone().unwrap_or_default();
            let season_year = entry.anime_season.as_ref().and_then(|s| s.year);
            let genres = entry.tags.clone().unwrap_or_default();

            let mut description = format!("Type: {}", entry.type_field);
            if let Some(syns) = &entry.synonyms {
                if !syns.is_empty() {
                    description.push_str(&format!("\nSynonyms: {}", syns.join(", ")));
                }
            }
            if !genres.is_empty() {
                description.push_str(&format!("\nTags: {}", genres.join(", ")));
            }

            AnimeInfo {
                id,
                title,
                status,
                episodes,
                description,
                cover_image_url,
                score: None,
                season_year,
                genres,
                sources: entry.sources.clone(),
            }
        })
        .collect()
}
