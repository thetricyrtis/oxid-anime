use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use tokio::sync::mpsc;
use zstd::stream::read::Decoder as ZstdDecoder;
use rusqlite::{Connection, params};
use crate::models::{OfflineAnimeEntry, DbUpdateResult};

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

/// Initializes the SQLite database and creates the required table if it doesn't exist.
pub fn init_db() -> rusqlite::Result<Connection> {
    let db_dir = "data/database";
    fs::create_dir_all(db_dir).ok();
    let db_path = format!("{}/oxid.db", db_dir);
    let conn = Connection::open(db_path)?;
    
    // Speed up bulk inserts and switch to WAL mode
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
        );"
    )?;
    
    Ok(conn)
}
/// Checks if the JSONL database exists and if not, extracts it from the .zst file.
/// If neither exists, it does nothing (update process handles downloading).
pub fn check_and_extract_db() -> io::Result<()> {
    let compressed_path = "data/compressed/anime-offline-database.jsonl.zst";
    let extracted_path = "data/database/anime-offline-database.jsonl";

    if !Path::new(extracted_path).exists() && Path::new(compressed_path).exists() {
        println!("Extracting offline database, please wait...");
        if let Some(parent) = Path::new(extracted_path).parent() {
            fs::create_dir_all(parent)?;
        }
        let f = File::open(compressed_path)?;
        let mut decoder = ZstdDecoder::new(f)?;
        let mut dest = File::create(extracted_path)?;
        io::copy(&mut decoder, &mut dest)?;
        println!("Database extracted successfully!");
    }
    Ok(())
}

pub fn populate_db_from_jsonl(conn: &mut Connection) -> io::Result<()> {
    let jsonl_path = "data/database/anime-offline-database.jsonl";
    if !Path::new(jsonl_path).exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "JSONL file not found"));
    }

    let file = File::open(jsonl_path)?;
    let reader = BufReader::new(file);

    // Start a transaction for bulk insert
    let tx = conn.transaction().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    tx.execute("DELETE FROM anime", []).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let mut count = 0;
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

                // Use transaction to insert
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
                
                count += 1;
            }
        }
    }

    // Commit all inserts at once
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
                
            // Fetch latest release info from cedya77 repository
            let release: GithubRelease = client
                .get("https://api.github.com/repos/cedya77/anime-offline-database/releases/latest")
                .send()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                .json()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                
            // Find the JSONL compressed asset
            let asset = release.assets.iter()
                .find(|a| a.name == "anime-offline-database.jsonl.zst")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Required database asset not found in latest release"))?;
                
            let download_url = &asset.browser_download_url;
            let tag_name = release.tag_name;
            
            let _ = tx.send(DbUpdateResult::Progress(format!("db_updating_downloading:{}", tag_name)));
            
            let mut response = client.get(download_url).send().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            
            let compressed_dir = "data/compressed";
            let compressed_temp_path = "data/compressed/anime-offline-database.jsonl.zst.tmp";
            fs::create_dir_all(compressed_dir)?;
            
            // Download directly to .tmp
            let mut out_file = File::create(compressed_temp_path)?;
            while let Some(chunk) = response.chunk().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))? {
                out_file.write_all(&chunk)?;
            }
            out_file.flush()?;
            
            let _ = tx.send(DbUpdateResult::Progress("db_updating_extracting".to_string()));
            
            let database_dir = "data/database";
            let decompressed_temp_path = "data/database/anime-offline-database.jsonl.tmp";
            fs::create_dir_all(database_dir)?;
            
            // Decompress to .tmp
            let f = File::open(compressed_temp_path)?;
            let mut decoder = ZstdDecoder::new(f)?;
            let mut dest = File::create(decompressed_temp_path)?;
            io::copy(&mut decoder, &mut dest)?;
            
            let final_compressed_path = "data/compressed/anime-offline-database.jsonl.zst";
            let final_decompressed_path = "data/database/anime-offline-database.jsonl";
            
            // Atomically rename files
            fs::rename(compressed_temp_path, final_compressed_path)?;
            fs::rename(decompressed_temp_path, final_decompressed_path)?;
            
            // Populate SQLite from the newly extracted JSONL
            if let Ok(mut conn) = init_db() {
                let _ = populate_db_from_jsonl(&mut conn);
            }
            
            fs::write("data/database/db_version.txt", &tag_name)?;
            
            Ok::<String, io::Error>(tag_name)
        };
        
        match run_update.await {
            Ok(tag) => {
                let _ = tx.send(DbUpdateResult::Success(tag));
            }
            Err(e) => {
                let _ = tx.send(DbUpdateResult::Error(e.to_string()));
            }
        }
    });
}
