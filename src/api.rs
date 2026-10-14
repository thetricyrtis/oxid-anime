use crate::models::*;
use serde_json::json;

pub async fn search_anime(query: &str) -> Result<Vec<AnimeInfo>, Box<dyn std::error::Error>> {
    let url = "https://graphql.anilist.co";
    let client = reqwest::Client::new();

    // 1. GraphQL sorgusunu hazırla
    let query_body = json!({
        "query": r#"
            query ($search: String) {
                Page(perPage: 10) {
                    media(search: $search, type: ANIME) {
                        id
                        title { english romaji }
                        episodes
                        status
                        description
                        coverImage { large }
                        averageScore
                        seasonYear
                        genres
                    }
                }
            }
        "#,
        "variables": { "search": query }
    });

    // 2. İsteği gönder ve yanıtı bekle
    let response = client.post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&query_body)
        .send()
        .await?
        .json::<ApiResponse>()
        .await?;

    // 3. API modelini uygulamanın güncel AnimeInfo modeline dönüştür
    let animeler = response.data.page.media.into_iter().map(|m| {
        let id = m.id.unwrap_or(0);
        let title = m.title.english.unwrap_or(m.title.romaji);
        let status = m.status.unwrap_or_else(|| "UNKNOWN".to_string());
        let description = clean_description(&m.description.unwrap_or_else(|| "Açıklama yok.".to_string()));
        let genres = m.genres.unwrap_or_default();

        AnimeInfo {
            id,
            title,
            status,
            episodes: m.episodes,
            description,
            cover_image_url: m.cover_image.large,
            score: m.average_score,
            season_year: m.season_year,
            genres,
            sources: Vec::new(), // Yerel SQLite eşleşmeleri için boş liste ile başlatıyoruz
        }
    }).collect();

    Ok(animeler)
}

/// HTML etiketlerini ve satır sonlarını temizleyen yardımcı fonksiyon
fn clean_description(input: &str) -> String {
    input.replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<i>", "")
        .replace("</i>", "")
        .replace("<b>", "")
        .replace("</b>", "")
}
