mod draw;
mod input;
mod playback;

use anyhow::Result;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::time::Instant;

use crate::bandcamp::client::BandcampClient;
use crate::bandcamp::models::{Album, AuthData, Track};
use crate::events::AppEvent;
use crate::library::{self, DownloadEvent, LibraryIndex};
use crate::player::engine::{AudioEngine, PlayerEvent};
use crate::player::queue::PlayQueue;
use crate::{auth, cache};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    Collection,
    Album,
    Downloaded,
    Settings,
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

pub struct App {
    pub screen: AppScreen,
    pub view: View,
    pub albums: Vec<Album>,
    pub queue: PlayQueue,
    pub collection_state: ListState,
    pub album_state: ListState,
    pub downloaded_state: ListState,
    pub library: LibraryIndex,
    pub download_rx: Vec<tokio::sync::mpsc::UnboundedReceiver<DownloadEvent>>,
    pub selected_album_idx: Option<usize>,
    pub is_paused: bool,
    pub meta_scroll: Option<usize>,
    pub elapsed: f64,
    pub play_started: Option<Instant>,
    pub pause_accumulated: f64,
    pub collection_filter: String,
    pub album_filter: String,
    pub downloaded_filter: String,
    pub collection_filtered_indices: Vec<usize>,
    pub album_filtered_indices: Vec<usize>,
    pub downloaded_filtered_indices: Vec<usize>,
    pub filter_mode: bool,
    pub status_msg: String,
    pub should_quit: bool,
    pub dirty: bool,
    pub auth: Option<AuthData>,
    pub login_step: LoginStep,
    pub art_protocol: Option<StatefulProtocol>,
    pub art_picker: Option<Picker>,
    pub(crate) mp3_rx: Option<tokio::sync::oneshot::Receiver<Result<Vec<u8>>>>,
    pub(crate) art_rx: Option<tokio::sync::oneshot::Receiver<Option<image::DynamicImage>>>,
    pub(crate) engine: Option<AudioEngine>,
    pub(crate) client: Option<BandcampClient>,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: AppScreen::Login,
            view: View::Collection,
            albums: Vec::new(),
            queue: PlayQueue::new(),
            collection_state: ListState::default(),
            album_state: ListState::default(),
            downloaded_state: ListState::default(),
            library: LibraryIndex::load().unwrap_or_else(|_| LibraryIndex::new()),
            download_rx: Vec::new(),
            selected_album_idx: None,
            is_paused: false,
            meta_scroll: None,
            elapsed: 0.0,
            play_started: None,
            pause_accumulated: 0.0,
            collection_filter: String::new(),
            album_filter: String::new(),
            downloaded_filter: String::new(),
            collection_filtered_indices: Vec::new(),
            album_filtered_indices: Vec::new(),
            downloaded_filtered_indices: Vec::new(),
            filter_mode: false,
            status_msg: String::new(),
            should_quit: false,
            dirty: true,
            auth: None,
            login_step: LoginStep::Prompt,
            art_protocol: None,
            art_picker: Picker::from_query_stdio().ok(),
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
            self.recompute_all_filters();
            self.screen = AppScreen::Main;
            if !self.albums.is_empty() {
                self.collection_state.select(Some(0));
            }
            self.status_msg = format!("Loaded {} albums from cache", self.albums.len());
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
        self.recompute_all_filters();

        self.screen = AppScreen::Main;
        if !self.albums.is_empty() {
            self.collection_state.select(Some(0));
        }
        self.status_msg = format!("Loaded {} albums", self.albums.len());
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
        // Blinking cursor in search bar needs periodic redraws
        if self.filter_mode {
            self.dirty = true;
        }

        if let Some(started) = self.play_started {
            if !self.is_paused {
                let new_elapsed = started.elapsed().as_secs_f64() - self.pause_accumulated;
                if (new_elapsed - self.elapsed).abs() > 0.1 {
                    self.elapsed = new_elapsed;
                    self.dirty = true;
                }
            }
        }

        // Check for player events
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

        // Check for MP3 download result
        if let Some(ref mut rx) = self.mp3_rx {
            if let Ok(result) = rx.try_recv() {
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
        }

        // Check for art download result
        if let Some(ref mut rx) = self.art_rx {
            if let Ok(result) = rx.try_recv() {
                self.art_rx = None;
                if let Some(img) = result {
                    if let Some(ref mut picker) = self.art_picker {
                        self.art_protocol = Some(picker.new_resize_protocol(img));
                        self.dirty = true;
                    }
                }
            }
        }

        // Poll download progress
        let mut completed_indices = Vec::new();
        for (i, rx) in self.download_rx.iter_mut().enumerate() {
            while let Ok(event) = rx.try_recv() {
                match event {
                    DownloadEvent::TrackDone { item_id, track_num } => {
                        if let Some(album) = self.library.albums.get_mut(&item_id) {
                            if let Some(track) =
                                album.tracks.iter_mut().find(|t| t.track_num == track_num)
                            {
                                track.downloaded = true;
                            }
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

    pub(crate) fn active_filter(&self) -> &str {
        match self.view {
            View::Collection => &self.collection_filter,
            View::Album => &self.album_filter,
            View::Downloaded => &self.downloaded_filter,
            _ => "",
        }
    }

    pub(crate) fn active_filter_mut(&mut self) -> &mut String {
        match self.view {
            View::Collection => &mut self.collection_filter,
            View::Album => &mut self.album_filter,
            View::Downloaded => &mut self.downloaded_filter,
            _ => &mut self.collection_filter,
        }
    }

    /// Recompute cached filtered indices for the collection view.
    pub(crate) fn recompute_collection_filter(&mut self) {
        if self.collection_filter.is_empty() {
            self.collection_filtered_indices = (0..self.albums.len()).collect();
        } else {
            let q = self.collection_filter.to_lowercase();
            self.collection_filtered_indices = self
                .albums
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    a.album_title.to_lowercase().contains(&q)
                        || a.artist_name.to_lowercase().contains(&q)
                })
                .map(|(i, _)| i)
                .collect();
        }
    }

    /// Recompute cached filtered indices for the album track view.
    pub(crate) fn recompute_album_filter(&mut self) {
        let tracks = self
            .selected_album_idx
            .and_then(|i| self.albums.get(i))
            .map(|a| &a.tracks[..])
            .unwrap_or(&[]);

        if self.album_filter.is_empty() {
            self.album_filtered_indices = (0..tracks.len()).collect();
        } else {
            let q = self.album_filter.to_lowercase();
            self.album_filtered_indices = tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.title.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
        }
    }

    /// Recompute cached filtered indices for the downloaded view.
    pub(crate) fn recompute_downloaded_filter(&mut self) {
        if self.downloaded_filter.is_empty() {
            self.downloaded_filtered_indices = (0..self.albums.len()).collect();
        } else {
            let q = self.downloaded_filter.to_lowercase();
            self.downloaded_filtered_indices = self
                .albums
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    a.album_title.to_lowercase().contains(&q)
                        || a.artist_name.to_lowercase().contains(&q)
                })
                .map(|(i, _)| i)
                .collect();
        }
    }

    /// Recompute the cached filter for the currently active view.
    pub(crate) fn recompute_active_filter(&mut self) {
        match self.view {
            View::Collection => self.recompute_collection_filter(),
            View::Album => self.recompute_album_filter(),
            View::Downloaded => self.recompute_downloaded_filter(),
            View::Settings => {}
        }
    }

    /// Recompute all cached filters (call after albums change).
    pub(crate) fn recompute_all_filters(&mut self) {
        self.recompute_collection_filter();
        self.recompute_album_filter();
        self.recompute_downloaded_filter();
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
                    duration: 0.0,
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
