use std::collections::HashMap;
use tokio::sync::mpsc;
use ratatui::layout::Rect;
use ratatui_image::protocol::Protocol;
use ratatui_image::picker::Picker;
use ratatui_image::FontSize;
use crate::models::AnimeInfo;
use anyhow::Result;

pub struct ImageManager {
    picker: Picker,
    cache: HashMap<String, Protocol>,
    tx: mpsc::UnboundedSender<(u64, String, Protocol)>,
    rx: mpsc::UnboundedReceiver<(u64, String, Protocol)>,
}

impl ImageManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<(u64, String, Protocol)>();
        let picker = Picker::from_query_stdio()
            .unwrap_or_else(|_| Picker::from_fontsize(FontSize::default()));
        ImageManager {
            picker,
            cache: HashMap::new(),
            tx,
            rx,
        }
    }

    pub fn try_recv(&mut self) -> Result<(u64, String, Protocol), mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }

    pub fn cache_insert(&mut self, url: String, protocol: Protocol) {
        self.cache.insert(url, protocol);
    }

    pub fn cache_get(&self, url: &str) -> Option<&Protocol> {
        self.cache.get(url)
    }

    // İŞTE UNUTTUĞUMUZ FONKSİYON BURADA :)
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub fn request_image(&self, list: &[AnimeInfo], selected: Option<usize>) {
        if let Some(idx) = selected {
            if let Some(anime) = list.get(idx) {
                let url = anime.cover_image_url.clone();
                if !self.cache.contains_key(&url) {
                    self.spawn_load(url, 0);
                }
            }
        }
    }

    pub fn preload_images(&self, list: &[AnimeInfo], count: usize, search_id: u64) {
        for anime in list.iter().take(count) {
            let url = anime.cover_image_url.clone();
            if !self.cache.contains_key(&url) {
                self.spawn_load(url, search_id);
            }
        }
    }

    fn spawn_load(&self, url: String, search_id: u64) {
        let tx = self.tx.clone();
        let picker = self.picker.clone();
        tokio::spawn(async move {
            let result = async {
                // URL UYUMLULUK KORUMASI: AniList yarım url dönerse başına cdn adresini ekler
                let target_url = if !url.starts_with("http://") && !url.starts_with("https://") {
                    format!("https://s4.anilist.co{}", url)
                } else {
                    url.clone()
                };

                let resp = reqwest::get(&target_url).await?;
                let data = resp.bytes().await?;
                let img = image::load_from_memory(&data)?;
                let rect = Rect::new(0, 0, 40, 20);
                let proto = picker.new_protocol(img, rect, ratatui_image::Resize::Fit(None))?;
                Ok::<_, anyhow::Error>(proto)
            }.await;

            if let Ok(proto) = result {
                let _ = tx.send((search_id, url, proto));
            }
        });
    }
}
