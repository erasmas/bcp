use anyhow::{Context, Result};

use super::{App, Column};
use crate::library;
use crate::player::engine::AudioEngine;

impl App {
    pub(crate) fn init_audio(&mut self) -> Result<()> {
        if self.engine.is_none() {
            self.engine = Some(AudioEngine::new()?);
        }
        Ok(())
    }

    pub(crate) fn start_playback(&mut self) {
        self.init_audio().ok();

        let Some(item) = self.queue.current_item() else {
            return;
        };

        let item_id = item.item_id;
        let track_num = item.track.track_num;
        self.dirty = true;

        // Check local library first
        if let Some(local_path) = self.library.track_path(item_id, track_num) {
            let (mp3_tx, mp3_rx) = tokio::sync::oneshot::channel();
            self.mp3_rx = Some(mp3_rx);
            tokio::spawn(async move {
                let result = tokio::fs::read(&local_path)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e));
                let _ = mp3_tx.send(result);
            });
            self.status_msg = format!(
                "Playing (local): {} - {}",
                item.artist_name, item.track.title
            );
        } else {
            let Some(ref url) = item.track.stream_url else {
                self.status_msg = "No stream URL for this track".to_string();
                return;
            };

            self.status_msg = format!("Buffering: {} - {}...", item.artist_name, item.track.title);

            let stream_url = url.clone();
            let album_url = item.item_url.clone();
            let track_num = item.track.track_num;
            let cookie = self
                .auth
                .as_ref()
                .map(|a| a.identity_cookie.clone())
                .unwrap_or_default();
            let (mp3_tx, mp3_rx) = tokio::sync::oneshot::channel();
            self.mp3_rx = Some(mp3_rx);
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                let result = async {
                    let mut req = client.get(&stream_url);
                    if !cookie.is_empty() {
                        req = req.header("Cookie", format!("identity={}", cookie));
                    }
                    let resp = req.send().await?;
                    if resp.status() == reqwest::StatusCode::GONE {
                        // Stream URL expired - re-fetch album page for fresh URLs
                        let cookie_header = if cookie.is_empty() {
                            String::new()
                        } else {
                            format!("identity={}", cookie)
                        };
                        let detail =
                            crate::bandcamp::client::BandcampClient::fetch_album_details_static(
                                &client,
                                &cookie_header,
                                &album_url,
                            )
                            .await?;
                        let track = detail
                            .tracks
                            .iter()
                            .find(|t| t.track_num == track_num)
                            .and_then(|t| t.stream_url.as_ref())
                            .context("No stream URL after refresh")?;
                        let resp = client.get(track).send().await?;
                        let bytes = resp.bytes().await?;
                        return Ok(bytes.to_vec());
                    }
                    let bytes = resp.bytes().await?;
                    Ok(bytes.to_vec()) as Result<Vec<u8>>
                }
                .await;
                let _ = mp3_tx.send(result);
            });
        }

        // Load album art - try local cover.jpg first, then fetch from network
        let artist_name = item.artist_name.clone();
        let album_title = item.album_title.clone();
        let art_url = item.art_url.clone();
        let (art_tx, art_rx) = tokio::sync::oneshot::channel();
        self.art_rx = Some(art_rx);
        tokio::spawn(async move {
            // Check local cover first
            if crate::library::has_cover(&artist_name, &album_title)
                && let Ok(base) = crate::config::library_dir()
            {
                let path = base
                    .join(crate::library::sanitize_for_path(&artist_name))
                    .join(crate::library::sanitize_for_path(&album_title))
                    .join("cover.jpg");
                if let Ok(img) = image::open(&path) {
                    let _ = art_tx.send(Some(img));
                    return;
                }
            }
            // Fall back to network and save locally
            if let Some(ref art) = art_url
                && crate::library::download_cover(&artist_name, &album_title, art)
                    .await
                    .is_ok()
                && let Ok(base) = crate::config::library_dir()
            {
                let path = base
                    .join(crate::library::sanitize_for_path(&artist_name))
                    .join(crate::library::sanitize_for_path(&album_title))
                    .join("cover.jpg");
                if let Ok(img) = image::open(&path) {
                    let _ = art_tx.send(Some(img));
                    return;
                }
            }
            let _ = art_tx.send(None);
        });
    }

    pub(crate) fn play_next(&mut self) {
        if self.queue.next().is_some() {
            self.start_playback();
            if let Some(idx) = self.queue.current {
                self.track_state.select(Some(idx));
            }
        } else {
            self.status_msg = "End of queue".to_string();
            self.play_started = None;
        }
    }

    pub(crate) fn play_prev(&mut self) {
        if self.queue.prev().is_some() {
            self.start_playback();
            if let Some(idx) = self.queue.current {
                self.track_state.select(Some(idx));
            }
        }
    }

    pub(crate) fn seek_by(&mut self, delta: f64) {
        let Some(ref engine) = self.engine else {
            return;
        };
        let Some(item) = self.queue.current_item() else {
            return;
        };
        if self.play_started.is_none() {
            return;
        };

        let duration = item.track.duration;
        let new_pos = (self.elapsed + delta).clamp(0.0, duration);
        if engine
            .seek(std::time::Duration::from_secs_f64(new_pos))
            .is_ok()
        {
            // Adjust play_started so elapsed reads new_pos going forward
            self.play_started =
                Some(std::time::Instant::now() - std::time::Duration::from_secs_f64(new_pos));
            self.elapsed = new_pos;
            self.dirty = true;
        }
    }

    pub(crate) fn toggle_pause(&mut self) -> Result<()> {
        if let Some(ref engine) = self.engine {
            if self.is_paused {
                engine.resume()?;
                self.is_paused = false;
                // Rebase play_started so elapsed resumes from where it was frozen.
                self.play_started = Some(
                    std::time::Instant::now() - std::time::Duration::from_secs_f64(self.elapsed),
                );
            } else {
                engine.pause()?;
                self.is_paused = true;
            }
        }
        Ok(())
    }

    /// Resolve the download page URL for an album by its sale_item_id.
    fn download_url_for(&self, album: &crate::bandcamp::models::Album) -> Option<String> {
        // Try sale_item_id first, then item_id (which is often the sale_item_id
        // due to how fetch_full_collection prefers sale_item_id as the id)
        if let Some(sale_id) = album.sale_item_id
            && let Some(url) = self.redownload_urls.get(&sale_id)
        {
            return Some(url.clone());
        }
        self.redownload_urls.get(&album.item_id).cloned()
    }

    /// Context-sensitive download based on focused column.
    pub(crate) async fn download_context(&mut self) {
        match self.focus {
            Column::Artists => self.download_artist_albums().await,
            Column::Albums => self.download_selected_album().await,
            Column::Tracks | Column::Queue => self.download_selected_album().await,
        }
    }

    async fn download_artist_albums(&mut self) {
        let artist_albums = self.current_artist_album_indices();
        if artist_albums.is_empty() {
            return;
        }

        let cookie = self
            .auth
            .as_ref()
            .map(|a| a.identity_cookie.clone())
            .unwrap_or_default();
        let mut count = 0;

        // Fetch details for albums that need it
        for &idx in &artist_albums {
            if self.albums[idx].tracks.is_empty()
                && !self.library.is_downloaded(self.albums[idx].item_id)
            {
                let url = self.albums[idx].item_url.clone();
                self.status_msg = format!(
                    "Fetching: {} - {}...",
                    self.albums[idx].artist_name, self.albums[idx].album_title
                );
                self.dirty = true;
                if let Some(ref client) = self.client
                    && let Ok(detail) = client.fetch_album_details(&url).await
                {
                    self.albums[idx].tracks = detail.tracks;
                    self.albums[idx].about = detail.about;
                    self.albums[idx].credits = detail.credits;
                    self.albums[idx].release_date = detail.release_date;
                }
            }
        }

        // Download each album
        for &idx in &artist_albums {
            let album = &self.albums[idx];
            if album.tracks.is_empty() || self.library.is_downloaded(album.item_id) {
                continue;
            }

            let entry = library::prepare_album_entry(album);
            self.library.albums.insert(album.item_id, entry);

            let dl_url = self.download_url_for(album);
            if let Ok(rx) = library::download_album(album, &cookie, dl_url) {
                self.download_rx.push(rx);
                count += 1;
            }
        }

        let _ = self.library.save();
        if count > 0 {
            let artist = self.selected_artist_name().unwrap_or("").to_string();
            self.status_msg = format!("Downloading {} albums for {}", count, artist);
        } else {
            self.status_msg = "All albums already downloaded".to_string();
        }
        self.dirty = true;
    }

    async fn download_selected_album(&mut self) {
        let Some(idx) = self.selected_album_idx else {
            return;
        };

        if idx >= self.albums.len() {
            return;
        }

        if self.library.is_downloaded(self.albums[idx].item_id) {
            self.status_msg = "Already downloaded".to_string();
            return;
        }

        if self.albums[idx].tracks.is_empty() {
            self.status_msg = "Fetching album details...".to_string();
            self.dirty = true;
            let url = self.albums[idx].item_url.clone();
            if let Some(ref client) = self.client {
                match client.fetch_album_details(&url).await {
                    Ok(detail) => {
                        self.albums[idx].tracks = detail.tracks;
                        self.albums[idx].about = detail.about;
                        self.albums[idx].credits = detail.credits;
                        self.albums[idx].release_date = detail.release_date;
                    }
                    Err(e) => {
                        self.status_msg = format!("Failed to fetch album: {}", e);
                        return;
                    }
                }
            }
        }

        let album = &self.albums[idx];
        let cookie = self
            .auth
            .as_ref()
            .map(|a| a.identity_cookie.as_str())
            .unwrap_or("");
        let entry = library::prepare_album_entry(album);
        self.library.albums.insert(album.item_id, entry);
        let _ = self.library.save();

        let dl_url = self.download_url_for(album);
        match library::download_album(album, cookie, dl_url) {
            Ok(rx) => {
                self.download_rx.push(rx);
                self.status_msg =
                    format!("Downloading: {} - {}", album.artist_name, album.album_title);
            }
            Err(e) => {
                self.status_msg = format!("Download error: {}", e);
            }
        }
        self.dirty = true;
    }

    pub(crate) async fn download_all_albums(&mut self) {
        let cookie = self
            .auth
            .as_ref()
            .map(|a| a.identity_cookie.clone())
            .unwrap_or_default();
        let mut count = 0;

        let indices_to_fetch: Vec<usize> = self
            .albums
            .iter()
            .enumerate()
            .filter(|(_, a)| a.tracks.is_empty() && !self.library.is_downloaded(a.item_id))
            .map(|(i, _)| i)
            .collect();

        for idx in indices_to_fetch {
            let url = self.albums[idx].item_url.clone();
            self.status_msg = format!(
                "Fetching: {} - {}...",
                self.albums[idx].artist_name, self.albums[idx].album_title
            );
            self.dirty = true;
            if let Some(ref client) = self.client
                && let Ok(detail) = client.fetch_album_details(&url).await
            {
                self.albums[idx].tracks = detail.tracks;
                self.albums[idx].about = detail.about;
                self.albums[idx].credits = detail.credits;
                self.albums[idx].release_date = detail.release_date;
            }
        }

        for album in &self.albums {
            if album.tracks.is_empty() || self.library.is_downloaded(album.item_id) {
                continue;
            }

            let entry = library::prepare_album_entry(album);
            self.library.albums.insert(album.item_id, entry);

            let dl_url = self.download_url_for(album);
            if let Ok(rx) = library::download_album(album, &cookie, dl_url) {
                self.download_rx.push(rx);
                count += 1;
            }
        }

        let _ = self.library.save();
        self.status_msg = format!("Downloading {} albums...", count);
        self.dirty = true;
    }
}
