mod draw;
mod input;
mod playback;

use anyhow::Result;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::collections::HashMap;
use std::time::Instant;

use crate::bandcamp::client::{AlbumDetail, BandcampClient};
use crate::bandcamp::models::{Album, AuthData, Track};
use crate::events::AppEvent;
use crate::library::{self, DownloadEvent, LibraryIndex};
use crate::player::engine::{AudioEngine, PlayerEvent};
use crate::player::queue::PlayQueue;
use crate::{auth, cache};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Column {
    Artists,
    Albums,
    Tracks,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppScreen {
    Login,
    Loading,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoginStep {
    Prompt,
    WaitingForBrowser,
    Extracting,
}

pub struct ArtistIndex {
    /// Sorted, deduplicated artist names (display casing)
    pub artists: Vec<String>,
    /// Map from lowercase artist name -> indices into App::albums
    pub albums_by_artist: HashMap<String, Vec<usize>>,
}

impl ArtistIndex {
    pub fn build(albums: &[Album]) -> Self {
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut albums_by_artist: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, album) in albums.iter().enumerate() {
            let key = album.artist_name.to_lowercase();
            seen.entry(key.clone())
                .or_insert_with(|| album.artist_name.clone());
            albums_by_artist.entry(key).or_default().push(i);
        }

        let mut artists: Vec<String> = seen.into_values().collect();
        artists.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

        ArtistIndex {
            artists,
            albums_by_artist,
        }
    }

    /// Get album indices for the artist at the given index in the sorted artists list.
    pub fn albums_for(&self, artist_idx: usize) -> &[usize] {
        if let Some(name) = self.artists.get(artist_idx) {
            let key = name.to_lowercase();
            self.albums_by_artist
                .get(&key)
                .map(|v| v.as_slice())
                .unwrap_or(&[])
        } else {
            &[]
        }
    }
}

pub struct App {
    pub screen: AppScreen,
    pub focus: Column,
    pub show_settings: bool,
    pub albums: Vec<Album>,
    pub artist_index: ArtistIndex,
    pub queue: PlayQueue,
    pub artist_state: ListState,
    pub album_state: ListState,
    pub track_state: ListState,
    pub library: LibraryIndex,
    pub download_rx: Vec<tokio::sync::mpsc::UnboundedReceiver<DownloadEvent>>,
    pub selected_album_idx: Option<usize>,
    pub is_paused: bool,
    pub meta_scroll: Option<usize>,
    pub elapsed: f64,
    pub play_started: Option<Instant>,
    pub pause_accumulated: f64,
    pub filter_text: String,
    pub artist_filtered: Vec<usize>,
    pub album_filtered: Vec<usize>,
    pub track_filtered: Vec<usize>,
    pub loading_tracks: bool,
    pub filter_mode: bool,
    pub status_msg: String,
    pub should_quit: bool,
    pub dirty: bool,
    pub auth: Option<AuthData>,
    pub login_step: LoginStep,
    pub art_protocol: Option<StatefulProtocol>,
    pub art_picker: Option<Picker>,
    pub(crate) tracks_rx: Option<(usize, tokio::sync::oneshot::Receiver<Result<AlbumDetail>>)>,
    pub(crate) detail_rx: Option<tokio::sync::mpsc::UnboundedReceiver<(usize, AlbumDetail)>>,
    pub(crate) prefetch_total: usize,
    pub(crate) prefetch_done: usize,
    pub(crate) mp3_rx: Option<tokio::sync::oneshot::Receiver<Result<Vec<u8>>>>,
    pub(crate) art_rx: Option<tokio::sync::oneshot::Receiver<Option<image::DynamicImage>>>,
    pub(crate) engine: Option<AudioEngine>,
    pub(crate) client: Option<BandcampClient>,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: AppScreen::Login,
            focus: Column::Artists,
            show_settings: false,
            albums: Vec::new(),
            artist_index: ArtistIndex {
                artists: Vec::new(),
                albums_by_artist: HashMap::new(),
            },
            queue: PlayQueue::new(),
            artist_state: ListState::default(),
            album_state: ListState::default(),
            track_state: ListState::default(),
            library: LibraryIndex::load().unwrap_or_else(|_| LibraryIndex::new()),
            download_rx: Vec::new(),
            selected_album_idx: None,
            is_paused: false,
            meta_scroll: None,
            elapsed: 0.0,
            play_started: None,
            pause_accumulated: 0.0,
            filter_text: String::new(),
            artist_filtered: Vec::new(),
            album_filtered: Vec::new(),
            track_filtered: Vec::new(),
            loading_tracks: false,
            filter_mode: false,
            status_msg: String::new(),
            should_quit: false,
            dirty: true,
            auth: None,
            login_step: LoginStep::Prompt,
            art_protocol: None,
            art_picker: Picker::from_query_stdio().ok(),
            tracks_rx: None,
            detail_rx: None,
            prefetch_total: 0,
            prefetch_done: 0,
            mp3_rx: None,
            art_rx: None,
            engine: None,
            client: None,
        }
    }

    pub async fn init(&mut self) -> Result<()> {
        if let Some(auth_data) = auth::load_auth()? {
            self.auth = Some(auth_data.clone());
            self.client = Some(BandcampClient::new(auth_data.identity_cookie.clone()));
            self.screen = AppScreen::Loading;
            self.status_msg = "Loading collection...".to_string();
        }
        Ok(())
    }

    pub async fn load_collection(&mut self) -> Result<()> {
        if let Some(cached) = cache::load_cached_collection()? {
            self.albums = cached;
            self.rebuild_artist_index();
            self.screen = AppScreen::Main;
            if !self.artist_index.artists.is_empty() {
                self.artist_state.select(Some(0));
                self.on_artist_changed();
            }
            self.hydrate_library_tracks();
            if self.detail_rx.is_none() {
                self.status_msg = format!("Loaded {} albums from cache", self.albums.len());
            }
            return Ok(());
        }

        let Some(ref auth_data) = self.auth else {
            anyhow::bail!("Not authenticated");
        };
        let Some(ref client) = self.client else {
            anyhow::bail!("No client");
        };

        let fan_id = match auth_data.fan_id {
            Some(id) => id,
            None => {
                self.status_msg = "Fetching fan info...".to_string();
                let (id, username) = client.fetch_fan_info().await?;
                let mut updated = auth_data.clone();
                updated.fan_id = Some(id);
                updated.username = Some(username);
                auth::save_auth(&updated)?;
                self.auth = Some(updated);
                id
            }
        };

        self.status_msg = "Fetching collection...".to_string();
        self.albums = client.fetch_full_collection(fan_id).await?;
        cache::save_collection_cache(&self.albums)?;
        self.rebuild_artist_index();

        self.screen = AppScreen::Main;
        if !self.artist_index.artists.is_empty() {
            self.artist_state.select(Some(0));
            self.on_artist_changed();
        }
        self.hydrate_library_tracks();
        if self.detail_rx.is_none() {
            self.status_msg = format!("Loaded {} albums", self.albums.len());
        }
        Ok(())
    }

    pub async fn handle_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::Key(key) => {
                self.handle_key(key).await?;
                self.dirty = true;
            }
            AppEvent::Resize => {
                self.dirty = true;
            }
            AppEvent::Tick => self.handle_tick().await?,
        }
        Ok(())
    }

    async fn handle_tick(&mut self) -> Result<()> {
        if self.filter_mode {
            self.dirty = true;
        }

        if let Some(started) = self.play_started
            && !self.is_paused
        {
            let new_elapsed = started.elapsed().as_secs_f64() - self.pause_accumulated;
            if (new_elapsed - self.elapsed).abs() > 0.1 {
                self.elapsed = new_elapsed;
                self.dirty = true;
            }
        }

        let mut track_finished = false;
        if let Some(ref mut engine) = self.engine {
            while let Ok(event) = engine.event_rx.try_recv() {
                match event {
                    PlayerEvent::Finished => track_finished = true,
                    PlayerEvent::Error(e) => {
                        self.status_msg = format!("Playback error: {}", e);
                        self.dirty = true;
                    }
                    _ => {}
                }
            }
        }
        if track_finished {
            self.play_next();
        }

        // Check for album detail fetch result
        if let Some((idx, ref mut rx)) = self.tracks_rx {
            if let Ok(result) = rx.try_recv() {
                self.tracks_rx = None;
                self.loading_tracks = false;
                match result {
                    Ok(detail) => {
                        self.albums[idx].tracks = detail.tracks;
                        self.albums[idx].about = detail.about;
                        self.albums[idx].credits = detail.credits;
                        self.albums[idx].release_date = detail.release_date;
                        // Save metadata.json
                        let album = &self.albums[idx];
                        let meta = library::AlbumMetadata {
                            item_id: album.item_id,
                            tracks: album
                                .tracks
                                .iter()
                                .map(|t| library::TrackMetadata {
                                    track_num: t.track_num,
                                    title: t.title.clone(),
                                    duration: t.duration,
                                    stream_url: t.stream_url.clone(),
                                })
                                .collect(),
                            about: album.about.clone(),
                            credits: album.credits.clone(),
                            release_date: album.release_date.clone(),
                        };
                        let _ = library::save_album_metadata(
                            &album.artist_name,
                            &album.album_title,
                            &meta,
                        );
                        self.recompute_track_filter();
                        if !self.track_filtered.is_empty() && self.track_state.selected().is_none()
                        {
                            self.track_state.select(Some(0));
                        }
                        self.status_msg = String::new();
                    }
                    Err(e) => {
                        self.status_msg = format!("Failed to load album: {}", e);
                    }
                }
                self.dirty = true;
            }
        }

        // Check for background album detail fetches (duration hydration)
        if let Some(ref mut rx) = self.detail_rx {
            let mut got_any = false;
            let mut done = false;
            loop {
                match rx.try_recv() {
                    Ok((idx, detail)) => {
                        self.albums[idx].tracks = detail.tracks;
                        self.albums[idx].about = detail.about;
                        self.albums[idx].credits = detail.credits;
                        self.albums[idx].release_date = detail.release_date;
                        // Save metadata.json to disk
                        let album = &self.albums[idx];
                        let meta = library::AlbumMetadata {
                            item_id: album.item_id,
                            tracks: album
                                .tracks
                                .iter()
                                .map(|t| library::TrackMetadata {
                                    track_num: t.track_num,
                                    title: t.title.clone(),
                                    duration: t.duration,
                                    stream_url: t.stream_url.clone(),
                                })
                                .collect(),
                            about: album.about.clone(),
                            credits: album.credits.clone(),
                            release_date: album.release_date.clone(),
                        };
                        let _ = library::save_album_metadata(
                            &album.artist_name,
                            &album.album_title,
                            &meta,
                        );
                        // Update durations in library for downloaded albums
                        let item_id = album.item_id;
                        if let Some(dl) = self.library.albums.get_mut(&item_id) {
                            for track in &mut dl.tracks {
                                if let Some(t) = self.albums[idx]
                                    .tracks
                                    .iter()
                                    .find(|t| t.track_num == track.track_num)
                                {
                                    track.duration = t.duration;
                                }
                            }
                        }
                        self.prefetch_done += 1;
                        self.status_msg = format!(
                            "syncing album metadata ({}/{})...",
                            self.prefetch_done, self.prefetch_total
                        );
                        got_any = true;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                }
            }
            if got_any {
                let _ = self.library.save();
                self.recompute_track_filter();
                self.dirty = true;
            }
            if done {
                self.detail_rx = None;
                let _ = cache::save_collection_cache(&self.albums);
                self.status_msg = String::new();
                self.dirty = true;
            }
        }

        if let Some(ref mut rx) = self.mp3_rx
            && let Ok(result) = rx.try_recv()
        {
            self.mp3_rx = None;
            match result {
                Ok(data) => {
                    if let Some(ref engine) = self.engine {
                        engine.play(data)?;
                        self.is_paused = false;
                        self.play_started = Some(Instant::now());
                        self.pause_accumulated = 0.0;
                        self.elapsed = 0.0;
                        self.meta_scroll = None;
                        self.status_msg = String::new();
                        self.dirty = true;
                    }
                }
                Err(e) => {
                    self.status_msg = format!("Stream error: {}", e);
                    self.dirty = true;
                }
            }
        }

        if let Some(ref mut rx) = self.art_rx
            && let Ok(result) = rx.try_recv()
        {
            self.art_rx = None;
            if let Some(img) = result
                && let Some(ref mut picker) = self.art_picker
            {
                self.art_protocol = Some(picker.new_resize_protocol(img));
                self.dirty = true;
            }
        }

        let mut completed_indices = Vec::new();
        for (i, rx) in self.download_rx.iter_mut().enumerate() {
            while let Ok(event) = rx.try_recv() {
                match event {
                    DownloadEvent::TrackDone { item_id, track_num } => {
                        if let Some(album) = self.library.albums.get_mut(&item_id)
                            && let Some(track) =
                                album.tracks.iter_mut().find(|t| t.track_num == track_num)
                        {
                            track.downloaded = true;
                        }
                        self.dirty = true;
                    }
                    DownloadEvent::AlbumDone { item_id } => {
                        if let Some(album) = self.library.albums.get_mut(&item_id) {
                            album.status = library::AlbumDownloadStatus::Complete;
                            self.status_msg = format!(
                                "Downloaded: {} - {}",
                                album.artist_name, album.album_title
                            );
                        }
                        let _ = self.library.save();
                        completed_indices.push(i);
                        self.dirty = true;
                    }
                    DownloadEvent::Error { item_id, msg } => {
                        self.status_msg = format!("Download error ({}): {}", item_id, msg);
                        self.dirty = true;
                    }
                }
            }
        }
        for i in completed_indices.into_iter().rev() {
            self.download_rx.remove(i);
        }

        Ok(())
    }

    // -- Artist index --

    pub(crate) fn rebuild_artist_index(&mut self) {
        self.artist_index = ArtistIndex::build(&self.albums);
        self.recompute_artist_filter();
    }

    /// Populate tracks from local metadata files and library, then prefetch
    /// track metadata from the API for all remaining albums in the background.
    pub(crate) fn hydrate_library_tracks(&mut self) {
        // Populate tracks from metadata.json or library data
        for album in self.albums.iter_mut() {
            if !album.tracks.is_empty() {
                continue;
            }
            // Try metadata.json first
            if let Some(meta) = library::load_album_metadata(&album.artist_name, &album.album_title)
            {
                album.tracks = meta
                    .tracks
                    .iter()
                    .map(|t| Track {
                        title: t.title.clone(),
                        track_num: t.track_num,
                        duration: t.duration,
                        stream_url: t.stream_url.clone(),
                    })
                    .collect();
                album.about = meta.about;
                album.credits = meta.credits;
                album.release_date = meta.release_date;
                continue;
            }
            // Fall back to library index
            if let Some(dl) = self.library.albums.get(&album.item_id) {
                album.tracks = dl
                    .tracks
                    .iter()
                    .map(|t| Track {
                        title: t.title.clone(),
                        track_num: t.track_num,
                        duration: t.duration,
                        stream_url: None,
                    })
                    .collect();
            }
        }

        // Download missing covers in the background
        let needs_covers: Vec<(String, String, String)> = self
            .albums
            .iter()
            .filter(|a| a.art_url.is_some() && !library::has_cover(&a.artist_name, &a.album_title))
            .map(|a| {
                (
                    a.artist_name.clone(),
                    a.album_title.clone(),
                    a.art_url.clone().unwrap(),
                )
            })
            .collect();

        if !needs_covers.is_empty() {
            tokio::spawn(async move {
                for (artist, album_title, art_url) in needs_covers {
                    let _ = library::download_cover(&artist, &album_title, &art_url).await;
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            });
        }

        // Prefetch track data for all albums that still have empty tracks
        let needs_fetch: Vec<(usize, String, String, String, Option<String>)> = self
            .albums
            .iter()
            .enumerate()
            .filter(|(_, a)| a.tracks.is_empty() && !a.item_url.is_empty())
            .map(|(i, a)| {
                (
                    i,
                    a.item_url.clone(),
                    a.artist_name.clone(),
                    a.album_title.clone(),
                    a.art_url.clone(),
                )
            })
            .collect();

        if needs_fetch.is_empty() {
            return;
        }

        let Some(ref client) = self.client else {
            return;
        };

        let http = client.clone_http();
        let cookie = client.cookie_header();
        self.prefetch_total = needs_fetch.len();
        self.prefetch_done = 0;
        self.status_msg = format!("syncing album metadata (0/{})...", self.prefetch_total);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.detail_rx = Some(rx);

        tokio::spawn(async move {
            for (idx, url, artist, album_title, art_url) in needs_fetch {
                if let Ok(detail) =
                    crate::bandcamp::client::BandcampClient::fetch_album_details_static(
                        &http, &cookie, &url,
                    )
                    .await
                {
                    // Download cover art
                    if let Some(ref art) = art_url {
                        let _ = library::download_cover(&artist, &album_title, art).await;
                    }

                    if tx.send((idx, detail)).is_err() {
                        break;
                    }
                }
                // Rate limit to avoid overwhelming the Bandcamp API
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    }

    // -- Selection cascade --

    pub(crate) fn on_artist_changed(&mut self) {
        self.recompute_album_filter();
        if !self.album_filtered.is_empty() {
            self.album_state.select(Some(0));
        } else {
            self.album_state.select(None);
        }
        self.on_album_changed();
    }

    pub(crate) fn on_album_changed(&mut self) {
        // Determine selected album index in the global albums vec
        if self.artist_state.selected().is_some() {
            let artist_albums = self.current_artist_album_indices();
            if let Some(&filtered_idx) = self
                .album_filtered
                .get(self.album_state.selected().unwrap_or(0))
            {
                if let Some(&album_idx) = artist_albums.get(filtered_idx) {
                    self.selected_album_idx = Some(album_idx);
                } else {
                    self.selected_album_idx = None;
                }
            } else {
                self.selected_album_idx = None;
            }
        } else {
            self.selected_album_idx = None;
        }
        self.recompute_track_filter();
        if !self.track_filtered.is_empty() {
            self.track_state.select(Some(0));
        } else {
            self.track_state.select(None);
        }
    }

    /// Get the album indices (into self.albums) for the currently selected artist.
    pub(crate) fn current_artist_album_indices(&self) -> Vec<usize> {
        let Some(selected) = self.artist_state.selected() else {
            return Vec::new();
        };
        let Some(&filtered_artist) = self.artist_filtered.get(selected) else {
            return Vec::new();
        };
        self.artist_index.albums_for(filtered_artist).to_vec()
    }

    /// Get the selected artist name for display.
    pub(crate) fn selected_artist_name(&self) -> Option<&str> {
        let selected = self.artist_state.selected()?;
        let &filtered_idx = self.artist_filtered.get(selected)?;
        self.artist_index
            .artists
            .get(filtered_idx)
            .map(|s| s.as_str())
    }

    /// Get the selected album for display.
    pub(crate) fn selected_album(&self) -> Option<&Album> {
        self.selected_album_idx.and_then(|i| self.albums.get(i))
    }

    // -- Filters --

    pub(crate) fn recompute_artist_filter(&mut self) {
        if self.filter_text.is_empty() || self.focus != Column::Artists {
            self.artist_filtered = (0..self.artist_index.artists.len()).collect();
        } else {
            let q = self.filter_text.to_lowercase();
            self.artist_filtered = self
                .artist_index
                .artists
                .iter()
                .enumerate()
                .filter(|(_, name)| name.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
        }
    }

    pub(crate) fn recompute_album_filter(&mut self) {
        let artist_albums = self.current_artist_album_indices();
        if self.filter_text.is_empty() || self.focus != Column::Albums {
            self.album_filtered = (0..artist_albums.len()).collect();
        } else {
            let q = self.filter_text.to_lowercase();
            self.album_filtered = artist_albums
                .iter()
                .enumerate()
                .filter(|(_, album_idx)| {
                    let album_idx = **album_idx;
                    self.albums
                        .get(album_idx)
                        .is_some_and(|a| a.album_title.to_lowercase().contains(&q))
                })
                .map(|(i, _)| i)
                .collect();
        }
    }

    pub(crate) fn recompute_track_filter(&mut self) {
        let tracks = self
            .selected_album_idx
            .and_then(|i| self.albums.get(i))
            .map(|a| &a.tracks[..])
            .unwrap_or(&[]);

        if self.filter_text.is_empty() || self.focus != Column::Tracks {
            self.track_filtered = (0..tracks.len()).collect();
        } else {
            let q = self.filter_text.to_lowercase();
            self.track_filtered = tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.title.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
        }
    }

    pub(crate) fn recompute_active_filter(&mut self) {
        match self.focus {
            Column::Artists => {
                self.recompute_artist_filter();
                self.on_artist_changed();
            }
            Column::Albums => {
                self.recompute_album_filter();
                self.on_album_changed();
            }
            Column::Tracks => self.recompute_track_filter(),
        }
    }

    /// Populate albums list from the library index for offline mode.
    pub(crate) fn load_albums_from_library(&mut self) {
        for dl_album in self.library.albums.values() {
            if self.albums.iter().any(|a| a.item_id == dl_album.item_id) {
                continue;
            }
            let tracks = dl_album
                .tracks
                .iter()
                .map(|t| Track {
                    title: t.title.clone(),
                    track_num: t.track_num,
                    duration: t.duration,
                    stream_url: None,
                })
                .collect();
            self.albums.push(Album {
                item_id: dl_album.item_id,
                album_title: dl_album.album_title.clone(),
                artist_name: dl_album.artist_name.clone(),
                item_url: String::new(),
                art_url: None,
                date_added: None,
                tracks,
                about: None,
                credits: None,
                release_date: None,
            });
        }
    }
}
