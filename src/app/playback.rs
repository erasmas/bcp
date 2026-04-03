use anyhow::Result;

use super::App;
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
            let (mp3_tx, mp3_rx) = tokio::sync::oneshot::channel();
            self.mp3_rx = Some(mp3_rx);
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                let result = async {
                    let resp = client.get(&stream_url).send().await?;
                    let bytes = resp.bytes().await?;
                    Ok(bytes.to_vec()) as Result<Vec<u8>>
                }
                .await;
                let _ = mp3_tx.send(result);
            });
        }

        // Spawn art download in background
        let art_url = item.art_url.clone();
        if let Some(art) = art_url {
            let art_sized = art.replace("_16.", "_5.");
            let (art_tx, art_rx) = tokio::sync::oneshot::channel();
            self.art_rx = Some(art_rx);
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                let result = async {
                    let resp = client.get(&art_sized).send().await.ok()?;
                    let bytes = resp.bytes().await.ok()?;
                    image::load_from_memory(&bytes).ok()
                }
                .await;
                let _ = art_tx.send(result);
            });
        }
    }

    pub(crate) fn play_next(&mut self) {
        if self.queue.next().is_some() {
            self.start_playback();
            if let Some(idx) = self.queue.current {
                self.album_state.select(Some(idx));
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
                self.album_state.select(Some(idx));
            }
        }
    }

    pub(crate) fn toggle_pause(&mut self) -> Result<()> {
        if let Some(ref engine) = self.engine {
            if self.is_paused {
                engine.resume()?;
                self.is_paused = false;
            } else {
                engine.pause()?;
                self.is_paused = true;
            }
        }
        Ok(())
    }

    pub(crate) async fn download_selected_album(&mut self) {
        let selected = match self.view {
            super::View::Collection => self.collection_state.selected(),
            super::View::Downloaded => self.downloaded_state.selected(),
            _ => None,
        };

        let Some(selected) = selected else { return };

        // Map filtered index to actual album index
        let idx = match self.view {
            super::View::Collection => self.collection_filtered_indices.get(selected).copied(),
            super::View::Downloaded => self.downloaded_filtered_indices.get(selected).copied(),
            _ => Some(selected),
        };

        let Some(idx) = idx else { return };
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

        match library::download_album(album, cookie) {
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
            if let Some(ref client) = self.client {
                if let Ok(detail) = client.fetch_album_details(&url).await {
                    self.albums[idx].tracks = detail.tracks;
                    self.albums[idx].about = detail.about;
                    self.albums[idx].credits = detail.credits;
                    self.albums[idx].release_date = detail.release_date;
                }
            }
        }

        for album in &self.albums {
            if album.tracks.is_empty() || self.library.is_downloaded(album.item_id) {
                continue;
            }

            let entry = library::prepare_album_entry(album);
            self.library.albums.insert(album.item_id, entry);

            if let Ok(rx) = library::download_album(album, &cookie) {
                self.download_rx.push(rx);
                count += 1;
            }
        }

        let _ = self.library.save();
        self.status_msg = format!("Downloading {} albums...", count);
        self.dirty = true;
    }
}
