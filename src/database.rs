use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use zstd::stream::read::Decoder as ZstdDecoder;
use rusqlite::{Connection, params, OptionalExtension};
use crate::models::{OfflineAnimeEntry, DbUpdateResult, LibraryEntry};
use crate::paths;

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

fn path_err_to_io(e: paths::PathError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

/// Initializes the SQLite database and creates the required tables if they don't exist.
pub fn init_db() -> Result<Connection, Box<dyn std::error::Error>> {
    let db_path = paths::db_path()?;
    let conn = Connection::open(db_path)?;

    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = OFF;

         CREATE TABLE IF NOT EXISTS anime (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            status TEXT,
            episodes INTEGER,
            picture TEXT,
            type_field TEXT,
            season TEXT,
            year INTEGER,
            synonyms TEXT,
            tags TEXT,
            sources TEXT
        );

         CREATE TABLE IF NOT EXISTS library (
            anilist_id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            progress INTEGER DEFAULT 0,
            score INTEGER,
            updated_at INTEGER NOT NULL
         );"
    )?;

    Ok(conn)
}

/// Returns true if the `anime` table has no rows.
pub fn is_anime_table_empty(conn: &Connection) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM anime", [], |row| row.get(0))?;
    Ok(count == 0)
}

// --- Library CRUD Functions ---

pub fn add_or_update_library_entry(conn: &Connection, entry: &LibraryEntry) -> rusqlite::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO library (anilist_id, title, status, progress, score, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(anilist_id) DO UPDATE SET
            title = excluded.title,
            status = excluded.status,
            progress = excluded.progress,
            score = excluded.score,
            updated_at = excluded.updated_at",
        params![
            entry.anilist_id,
            entry.title,
            entry.status,
            entry.progress,
            entry.score,
            now
        ],
    )?;
    Ok(())
}

pub fn get_library_entry(conn: &Connection, anilist_id: u32) -> rusqlite::Result<Option<LibraryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT anilist_id, title, status, progress, score, updated_at
         FROM library WHERE anilist_id = ?1"
    )?;

    let entry = stmt.query_row([anilist_id], |row| {
        Ok(LibraryEntry {
            anilist_id: row.get(0)?,
            title: row.get(1)?,
            status: row.get(2)?,
            progress: row.get(3)?,
            score: row.get(4)?,
            updated_at: row.get(5)?,
        })
    }).optional()?;

    Ok(entry)
}

pub fn remove_library_entry(conn: &Connection, anilist_id: u32) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM library WHERE anilist_id = ?1", [anilist_id])?;
    Ok(())
}

pub fn get_all_library_entries(conn: &Connection) -> rusqlite::Result<Vec<LibraryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT anilist_id, title, status, progress, score, updated_at
         FROM library ORDER BY updated_at DESC"
    )?;

    let entries_iter = stmt.query_map([], |row| {
        Ok(LibraryEntry {
            anilist_id: row.get(0)?,
            title: row.get(1)?,
            status: row.get(2)?,
            progress: row.get(3)?,
            score: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;

    let mut entries = Vec::new();
    for entry in entries_iter {
        entries.push(entry?);
    }
    Ok(entries)
}

// --- Offline DB extraction & population ---

/// Checks if the JSONL database exists and if not, extracts it from the .zst file.
pub fn check_and_extract_db() -> io::Result<()> {
    let compressed_path = paths::compressed_path().map_err(path_err_to_io)?;
    let extracted_path = paths::jsonl_path().map_err(path_err_to_io)?;

    if !extracted_path.exists() && compressed_path.exists() {
        println!("Extracting offline database, please wait...");
        if let Some(parent) = extracted_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let f = File::open(&compressed_path)?;
        let mut decoder = ZstdDecoder::new(f)?;
        let mut dest = File::create(&extracted_path)?;
        io::copy(&mut decoder, &mut dest)?;
        println!("Database extracted successfully!");
    }
    Ok(())
}

pub fn populate_db_from_jsonl(conn: &mut Connection) -> io::Result<()> {
    let jsonl_path = paths::jsonl_path().map_err(path_err_to_io)?;
    if !jsonl_path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "JSONL file not found"));
    }

    let file = File::open(&jsonl_path)?;
    let reader = BufReader::new(file);

    let tx = conn.transaction().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    tx.execute("DELETE FROM anime", []).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(entry) = serde_json::from_str::<OfflineAnimeEntry>(&line) {
            if let Some(anilist_id) = entry.anilist_id() {
                let synonyms_json = serde_json::to_string(&entry.synonyms).unwrap_or_else(|_| "[]".to_string());
                let tags_json = serde_json::to_string(&entry.tags).unwrap_or_else(|_| "[]".to_string());
                let sources_json = serde_json::to_string(&entry.sources).unwrap_or_else(|_| "[]".to_string());

                let season = entry.anime_season.as_ref().map(|s| s.season.clone());
                let year = entry.anime_season.as_ref().and_then(|s| s.year);

                tx.execute(
                    "INSERT OR REPLACE INTO anime
                    (id, title, status, episodes, picture, type_field, season, year, synonyms, tags, sources)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        anilist_id as i64,
                        entry.title,
                        entry.status,
                        entry.episodes,
                        entry.picture,
                        entry.type_field,
                        season,
                        year,
                        synonyms_json,
                        tags_json,
                        sources_json
                    ],
                ).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            }
        }
    }

    tx.commit().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(())
}

/// Downloads the latest anime-offline-database.jsonl.zst and updates the local database.
pub fn spawn_db_update(tx: mpsc::UnboundedSender<DbUpdateResult>) {
    tokio::spawn(async move {
        let run_update = async {
            let _ = tx.send(DbUpdateResult::Progress("db_updating_checking".to_string()));

            let client = reqwest::Client::builder()
                .user_agent("oxid-anime")
                .build()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let release: GithubRelease = client
                .get("https://api.github.com/repos/cedya77/anime-offline-database/releases/latest")
                .send()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                .json()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let asset = release.assets.iter()
                .find(|a| a.name == "anime-offline-database.jsonl.zst")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Required database asset not found in latest release"))?;

            let download_url = &asset.browser_download_url;
            let tag_name = release.tag_name;

            let _ = tx.send(DbUpdateResult::Progress(format!("db_updating_downloading:{}", tag_name)));

            let mut response = client.get(download_url).send().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let compressed_path = paths::compressed_path().map_err(path_err_to_io)?;
            let compressed_temp_path = compressed_path.with_extension("zst.tmp");

            let mut out_file = File::create(&compressed_temp_path)?;
            while let Some(chunk) = response.chunk().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))? {
                out_file.write_all(&chunk)?;
            }
            out_file.flush()?;
            drop(out_file);

            let _ = tx.send(DbUpdateResult::Progress("db_updating_extracting".to_string()));

            let jsonl_path = paths::jsonl_path().map_err(path_err_to_io)?;
            let jsonl_temp_path = jsonl_path.with_extension("jsonl.tmp");

            let f = File::open(&compressed_temp_path)?;
            let mut decoder = ZstdDecoder::new(f)?;
            let mut dest = File::create(&jsonl_temp_path)?;
            io::copy(&mut decoder, &mut dest)?;
            drop(dest);

            fs::rename(&compressed_temp_path, &compressed_path)?;
            fs::rename(&jsonl_temp_path, &jsonl_path)?;

            if let Ok(mut conn) = init_db() {
                let _ = populate_db_from_jsonl(&mut conn);
            }

            let version_path = paths::version_path().map_err(path_err_to_io)?;
            fs::write(&version_path, &tag_name)?;

            Ok::<String, io::Error>(tag_name)
        };

        match run_update.await {
            Ok(tag) => { let _ = tx.send(DbUpdateResult::Success(tag)); }
            Err(e) => { let _ = tx.send(DbUpdateResult::Error(e.to_string())); }
        }
    });
}

pub fn get_anime_by_id(conn: &Connection, anilist_id: u32) -> rusqlite::Result<Option<crate::models::AnimeInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, status, episodes, picture, type_field, season, year, tags
         FROM anime WHERE id = ?1"
    )?;

    let result = stmt.query_row([anilist_id], |row| {
        let tags_json: String = row.get::<_, String>(8)?;
        let genres: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        Ok(crate::models::AnimeInfo {
            id: row.get(0)?,
            title: row.get(1)?,
            status: row.get::<_, Option<String>>(2)?.unwrap_or_else(|| "Unknown".to_string()),
            episodes: row.get(3)?,
            description: String::new(),
            cover_image_url: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            score: None,
            season_year: row.get(7)?,
            genres,
            sources: vec![],
        })
    }).optional()?;

    Ok(result)
}
