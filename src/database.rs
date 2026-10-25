use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use tokio::sync::mpsc;
use zstd::stream::read::Decoder as ZstdDecoder;
use crate::models::{OfflineAnimeEntry, DbUpdateResult};

#[derive(serde::Deserialize)]
struct OfflineDbWrapper {
    data: Vec<OfflineAnimeEntry>,
}

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

pub fn check_and_extract_db() -> io::Result<()> {
    let compressed_path = "data/compressed/anilist-minified.json.zst";
    let extracted_path = "data/database/anilist-minified.json";

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

pub fn load_offline_db() -> io::Result<Vec<OfflineAnimeEntry>> {
    let path = "data/database/anilist-minified.json";
    if !Path::new(path).exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    let wrapper: OfflineDbWrapper = serde_json::from_reader(reader)?;
    Ok(wrapper.data)
}

pub fn spawn_db_update(tx: mpsc::UnboundedSender<DbUpdateResult>) {
    tokio::spawn(async move {
        let run_update = async {
            let _ = tx.send(DbUpdateResult::Progress("db_updating_checking".to_string()));
            
            let client = reqwest::Client::builder()
                .user_agent("oxid-anime")
                .build()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                
            // Cedya77 reposuna geçildi
            let release: GithubRelease = client
                .get("https://api.github.com/repos/cedya77/anime-offline-database/releases/latest")
                .send()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                .json()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                
            // Anilist zst dosyası aranıyor
            let asset = release.assets.iter()
                .find(|a| a.name == "anilist-minified.json.zst")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Required database asset not found in latest release"))?;
                
            let download_url = &asset.browser_download_url;
            let tag_name = release.tag_name;
            
            let _ = tx.send(DbUpdateResult::Progress(format!("db_updating_downloading:{}", tag_name)));
            
            let mut response = client.get(download_url).send().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            
            let compressed_dir = "data/compressed";
            let compressed_temp_path = "data/compressed/anilist-minified.json.zst.tmp";
            fs::create_dir_all(compressed_dir)?;
            
            // Zaten .zst indirdiğimiz için tekrar şifrelemeye gerek yok, direkt yazıyoruz
            let mut out_file = File::create(compressed_temp_path)?;
            while let Some(chunk) = response.chunk().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))? {
                out_file.write_all(&chunk)?;
            }
            out_file.flush()?;
            
            let _ = tx.send(DbUpdateResult::Progress("db_updating_extracting".to_string()));
            
            let database_dir = "data/database";
            let decompressed_temp_path = "data/database/anilist-minified.json.tmp";
            fs::create_dir_all(database_dir)?;
            
            // Zstd ile açıyoruz
            let f = File::open(compressed_temp_path)?;
            let mut decoder = ZstdDecoder::new(f)?;
            let mut dest = File::create(decompressed_temp_path)?;
            io::copy(&mut decoder, &mut dest)?;
            
            let final_compressed_path = "data/compressed/anilist-minified.json.zst";
            let final_decompressed_path = "data/database/anilist-minified.json";
            
            fs::rename(compressed_temp_path, final_compressed_path)?;
            fs::rename(decompressed_temp_path, final_decompressed_path)?;
            
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
