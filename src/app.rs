use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    widgets::ListState,
    Frame,
};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;
use std::time::Instant;

use crate::auth;
use crate::bandcamp::client::BandcampClient;
use crate::bandcamp::models::{Album, AuthData};
use crate::cache;
use crate::events::AppEvent;
use crate::player::engine::{AudioEngine, PlayerEvent};
use crate::player::queue::{PlayQueue, QueueItem};
use crate::ui::theme;
use crate::ui::views::album::AlbumView;
use crate::ui::views::collection::CollectionView;
use crate::ui::views::now_playing::NowPlayingBar;
use crate::ui::views::queue_view::QueueView;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    Collection,
    Album,
    Queue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppScreen {
    Login,
    Loading,
    Main,
}

pub struct App {
    pub screen: AppScreen,
    pub view: View,
    pub albums: Vec<Album>,
    pub queue: PlayQueue,
    pub collection_state: ListState,
    pub album_state: ListState,
    pub queue_state: ListState,
    pub selected_album_idx: Option<usize>,
    pub is_paused: bool,
    pub meta_scroll: Option<usize>,  // None = auto-scroll, Some(n) = manual offset
    pub elapsed: f64,
    pub play_started: Option<Instant>,
    pub pause_accumulated: f64,
    pub filter: String,
    pub filter_mode: bool,
    pub status_msg: String,
    pub should_quit: bool,
    pub dirty: bool,
    pub auth: Option<AuthData>,
    pub login_step: LoginStep,
    pub art_protocol: Option<StatefulProtocol>,
    pub art_picker: Option<Picker>,
    mp3_rx: Option<tokio::sync::oneshot::Receiver<Result<Vec<u8>>>>,
    art_rx: Option<tokio::sync::oneshot::Receiver<Option<image::DynamicImage>>>,
    engine: Option<AudioEngine>,
    client: Option<BandcampClient>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoginStep {
    Prompt,
    WaitingForBrowser,
    Extracting,
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
            queue_state: ListState::default(),
            selected_album_idx: None,
            is_paused: false,
            meta_scroll: None,
            elapsed: 0.0,
            play_started: None,
            pause_accumulated: 0.0,
            filter: String::new(),
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
        // Try to load existing auth
        if let Some(auth_data) = auth::load_auth()? {
            self.auth = Some(auth_data.clone());
            self.client = Some(BandcampClient::new(auth_data.identity_cookie.clone()));
            self.screen = AppScreen::Loading;
            self.status_msg = "Loading collection...".to_string();
        }
        // else stay on Login screen

        Ok(())
    }

    pub async fn load_collection(&mut self) -> Result<()> {
        // Try cache first
        if let Some(cached) = cache::load_cached_collection()? {
            self.albums = cached;
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

                // Update stored auth with fan_id
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

        self.screen = AppScreen::Main;
        if !self.albums.is_empty() {
            self.collection_state.select(Some(0));
        }
        self.status_msg = format!("Loaded {} albums", self.albums.len());
        Ok(())
    }

    pub fn init_audio(&mut self) -> Result<()> {
        if self.engine.is_none() {
            self.engine = Some(AudioEngine::new()?);
        }
        Ok(())
    }

    pub async fn handle_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::Key(key) => {
                self.handle_key(key).await?;
                self.dirty = true;
            }
            AppEvent::Tick => self.handle_tick().await?,
        }
        Ok(())
    }

    async fn handle_tick(&mut self) -> Result<()> {
        // Update elapsed time
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
                    PlayerEvent::Finished => {
                        track_finished = true;
                    }
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

        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Handle Ctrl+C globally
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return Ok(());
        }

        match self.screen {
            AppScreen::Login => self.handle_login_key(key).await?,
            AppScreen::Loading => {} // No input during loading
            AppScreen::Main => {
                if self.filter_mode {
                    self.handle_filter_key(key)?;
                } else {
                    self.handle_main_key(key).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_login_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.login_step {
            LoginStep::Prompt => {
                if key.code == KeyCode::Enter {
                    auth::open_login_page()?;
                    self.login_step = LoginStep::WaitingForBrowser;
                    self.status_msg = "Browser opened — log in, then press Enter here".to_string();
                } else if key.code == KeyCode::Char('q') {
                    self.should_quit = true;
                }
            }
            LoginStep::WaitingForBrowser => {
                if key.code == KeyCode::Enter {
                    self.login_step = LoginStep::Extracting;
                    self.status_msg = "Extracting cookie from browser...".to_string();

                    match auth::extract_bandcamp_cookie()? {
                        Some(cookie) => {
                            let auth_data = AuthData {
                                identity_cookie: cookie.clone(),
                                fan_id: None,
                                username: None,
                            };
                            auth::save_auth(&auth_data)?;
                            self.auth = Some(auth_data);
                            self.client = Some(BandcampClient::new(cookie));
                            self.screen = AppScreen::Loading;
                            self.status_msg = "Authenticated! Loading collection...".to_string();
                        }
                        None => {
                            self.login_step = LoginStep::WaitingForBrowser;
                            self.status_msg =
                                "Could not find cookie — make sure you're logged in, then press Enter"
                                    .to_string();
                        }
                    }
                } else if key.code == KeyCode::Char('q') {
                    self.should_quit = true;
                }
            }
            LoginStep::Extracting => {}
        }

        Ok(())
    }

    async fn handle_main_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => match self.view {
                View::Album => self.view = View::Collection,
                View::Queue => self.view = View::Collection,
                _ => {}
            },
            KeyCode::Char('1') => self.view = View::Collection,
            KeyCode::Char('2') => {
                if self.selected_album_idx.is_some() {
                    self.view = View::Album;
                }
            }
            KeyCode::Char('3') => self.view = View::Queue,
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('g') => self.move_to_top(),
            KeyCode::Char('G') => self.move_to_bottom(),
            KeyCode::Enter => self.handle_enter().await?,
            KeyCode::Char(' ') => self.toggle_pause()?,
            KeyCode::Char('n') => self.play_next(),
            KeyCode::Char('p') => self.play_prev(),
            KeyCode::Char('J') => {
                // Scroll metadata text down
                let offset = self.meta_scroll.unwrap_or(0);
                self.meta_scroll = Some(offset + 1);
            }
            KeyCode::Char('K') => {
                // Scroll metadata text up
                let offset = self.meta_scroll.unwrap_or(0);
                self.meta_scroll = Some(offset.saturating_sub(1));
            }
            KeyCode::Tab => {
                // Toggle auto-scroll / manual
                if self.meta_scroll.is_some() {
                    self.meta_scroll = None; // back to auto
                } else {
                    self.meta_scroll = Some(0); // manual at top
                }
            }
            KeyCode::Char('s') => {
                self.queue.shuffle = !self.queue.shuffle;
                self.status_msg = format!(
                    "Shuffle: {}",
                    if self.queue.shuffle { "on" } else { "off" }
                );
            }
            KeyCode::Char('r') => match self.view {
                View::Collection => {
                    // Refresh collection
                    cache::invalidate_cache()?;
                    self.screen = AppScreen::Loading;
                    self.status_msg = "Refreshing collection...".to_string();
                }
                _ => {
                    self.queue.repeat = !self.queue.repeat;
                    self.status_msg = format!(
                        "Repeat: {}",
                        if self.queue.repeat { "on" } else { "off" }
                    );
                }
            },
            KeyCode::Char('/') => {
                self.filter_mode = true;
                self.filter.clear();
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.filter.clear();
            }
            KeyCode::Enter => {
                self.filter_mode = false;
                // Keep filter active
            }
            KeyCode::Backspace => {
                self.filter.pop();
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                // Reset selection when filter changes
                self.collection_state.select(Some(0));
            }
            _ => {}
        }
        Ok(())
    }

    fn move_selection(&mut self, delta: i32) {
        let (state, len) = match self.view {
            View::Collection => {
                let filtered_len = if self.filter.is_empty() {
                    self.albums.len()
                } else {
                    let q = self.filter.to_lowercase();
                    self.albums
                        .iter()
                        .filter(|a| {
                            a.album_title.to_lowercase().contains(&q)
                                || a.artist_name.to_lowercase().contains(&q)
                        })
                        .count()
                };
                (&mut self.collection_state, filtered_len)
            }
            View::Album => {
                let len = self
                    .selected_album_idx
                    .and_then(|i| self.albums.get(i))
                    .map(|a| a.tracks.len())
                    .unwrap_or(0);
                (&mut self.album_state, len)
            }
            View::Queue => (&mut self.queue_state, self.queue.items.len()),
        };

        if len == 0 {
            return;
        }

        let current = state.selected().unwrap_or(0);
        let new = if delta > 0 {
            (current + delta as usize).min(len - 1)
        } else {
            current.saturating_sub((-delta) as usize)
        };
        state.select(Some(new));
    }

    fn move_to_top(&mut self) {
        match self.view {
            View::Collection => self.collection_state.select(Some(0)),
            View::Album => self.album_state.select(Some(0)),
            View::Queue => self.queue_state.select(Some(0)),
        }
    }

    fn move_to_bottom(&mut self) {
        let len = match self.view {
            View::Collection => self.albums.len(),
            View::Album => self
                .selected_album_idx
                .and_then(|i| self.albums.get(i))
                .map(|a| a.tracks.len())
                .unwrap_or(0),
            View::Queue => self.queue.items.len(),
        };
        if len > 0 {
            match self.view {
                View::Collection => self.collection_state.select(Some(len - 1)),
                View::Album => self.album_state.select(Some(len - 1)),
                View::Queue => self.queue_state.select(Some(len - 1)),
            }
        }
    }

    async fn handle_enter(&mut self) -> Result<()> {
        match self.view {
            View::Collection => {
                let Some(selected) = self.collection_state.selected() else {
                    return Ok(());
                };

                // Map filtered index to actual album index
                let actual_idx = if self.filter.is_empty() {
                    selected
                } else {
                    let q = self.filter.to_lowercase();
                    let filtered: Vec<usize> = self
                        .albums
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| {
                            a.album_title.to_lowercase().contains(&q)
                                || a.artist_name.to_lowercase().contains(&q)
                        })
                        .map(|(i, _)| i)
                        .collect();
                    match filtered.get(selected) {
                        Some(&idx) => idx,
                        None => return Ok(()),
                    }
                };

                // Fetch tracks and metadata if not already loaded
                if self.albums[actual_idx].tracks.is_empty() {
                    self.status_msg = "Loading album...".to_string();
                    let url = self.albums[actual_idx].item_url.clone();
                    if let Some(ref client) = self.client {
                        match client.fetch_album_details(&url).await {
                            Ok(detail) => {
                                self.albums[actual_idx].tracks = detail.tracks;
                                self.albums[actual_idx].about = detail.about;
                                self.albums[actual_idx].credits = detail.credits;
                                self.albums[actual_idx].release_date = detail.release_date;
                            }
                            Err(e) => {
                                self.status_msg = format!("Failed to load album: {}", e);
                                return Ok(());
                            }
                        }
                    }
                }

                self.selected_album_idx = Some(actual_idx);
                self.album_state.select(Some(0));
                self.view = View::Album;
                self.status_msg = String::new();
            }
            View::Album => {
                let Some(album_idx) = self.selected_album_idx else {
                    return Ok(());
                };
                let Some(track_idx) = self.album_state.selected() else {
                    return Ok(());
                };

                let album = &self.albums[album_idx];

                // Build queue from album tracks starting at selected
                let items: Vec<QueueItem> = album
                    .tracks
                    .iter()
                    .map(|t| QueueItem {
                        track: t.clone(),
                        album_title: album.album_title.clone(),
                        artist_name: album.artist_name.clone(),
                        art_url: album.art_url.clone(),
                        about: album.about.clone(),
                        credits: album.credits.clone(),
                        release_date: album.release_date.clone(),
                    })
                    .collect();

                self.queue.replace_all(items, track_idx);
                self.start_playback();
            }
            View::Queue => {
                if let Some(selected) = self.queue_state.selected() {
                    self.queue.current = Some(selected);
                    self.start_playback();
                }
            }
        }
        Ok(())
    }

    fn start_playback(&mut self) {
        self.init_audio().ok();

        let Some(item) = self.queue.current_item() else {
            return;
        };

        let Some(ref url) = item.track.stream_url else {
            self.status_msg = "No stream URL for this track".to_string();
            return;
        };

        self.status_msg = format!("Buffering: {} - {}...", item.artist_name, item.track.title);
        self.dirty = true;

        // Spawn MP3 download in background
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

    fn play_next(&mut self) {
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

    fn play_prev(&mut self) {
        if self.queue.prev().is_some() {
            self.start_playback();
            if let Some(idx) = self.queue.current {
                self.album_state.select(Some(idx));
            }
        }
    }

    fn toggle_pause(&mut self) -> Result<()> {
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

    pub fn draw(&mut self, frame: &mut Frame) {
        match self.screen {
            AppScreen::Login => self.draw_login(frame),
            AppScreen::Loading => self.draw_loading(frame),
            AppScreen::Main => self.draw_main(frame),
        }
    }

    fn draw_login(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::vertical([
            Constraint::Percentage(30),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

        let logo = ratatui::widgets::Paragraph::new(" bcp - Bandcamp Player")
            .style(theme::title())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(logo, chunks[1]);

        let msg = match self.login_step {
            LoginStep::Prompt => "Press Enter to open Bandcamp login in your browser",
            LoginStep::WaitingForBrowser => {
                "Log in to Bandcamp in your browser, then press Enter here"
            }
            LoginStep::Extracting => "Extracting session cookie...",
        };
        let prompt = ratatui::widgets::Paragraph::new(msg)
            .style(theme::normal())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(prompt, chunks[2]);

        if !self.status_msg.is_empty() {
            let status = ratatui::widgets::Paragraph::new(self.status_msg.as_str())
                .style(theme::dim())
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(status, chunks[3]);
        }
    }

    fn draw_loading(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::vertical([
            Constraint::Percentage(40),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

        let msg = ratatui::widgets::Paragraph::new(self.status_msg.as_str())
            .style(theme::normal())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(msg, chunks[1]);
    }

    fn draw_main(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let np_height = NowPlayingBar::ideal_height(area.width);
        let chunks = Layout::vertical([
            Constraint::Length(np_height), // Now playing — sized to fit album art
            Constraint::Min(10),           // Main view
            Constraint::Length(1),         // Status bar
        ])
        .split(area);

        // Now playing bar
        let has_art = self.art_protocol.is_some();
        let now_playing = NowPlayingBar {
            current: self.queue.current_item(),
            is_paused: self.is_paused,
            elapsed: self.elapsed,
            has_art,
            meta_scroll: self.meta_scroll,
        };
        let np_area = chunks[0];
        frame.render_widget(now_playing, np_area);

        // Render album art inside the now-playing bar
        if let Some(ref mut protocol) = self.art_protocol {
            let art_rect = NowPlayingBar::art_area(np_area);
            let art_widget = StatefulImage::default().resize(ratatui_image::Resize::Scale(None));
            frame.render_stateful_widget(art_widget, art_rect, protocol);
        }

        // Main view
        match self.view {
            View::Collection => {
                let view = CollectionView {
                    albums: &self.albums,
                    filter: &self.filter,
                };
                frame.render_stateful_widget(view, chunks[1], &mut self.collection_state);
            }
            View::Album => {
                if let Some(idx) = self.selected_album_idx {
                    if let Some(album) = self.albums.get(idx) {
                        let playing_track = self
                            .queue
                            .current_item()
                            .map(|q| q.track.track_num);
                        let view = AlbumView {
                            album,
                            playing_track_num: playing_track,
                        };
                        frame.render_stateful_widget(view, chunks[1], &mut self.album_state);
                    }
                }
            }
            View::Queue => {
                let view = QueueView { queue: &self.queue };
                frame.render_stateful_widget(view, chunks[1], &mut self.queue_state);
            }
        }

        // Status bar: tabs on the right, hints on the left
        let status_area = chunks[2];

        // Split status bar: left for hints, right for tabs
        let status_chunks = Layout::horizontal([
            Constraint::Min(10),      // hints
            Constraint::Length(40),    // tabs
        ])
        .split(status_area);

        // Left: context hints or status message
        let hint_text = if self.filter_mode {
            format!(" /{}_ ", self.filter)
        } else if !self.status_msg.is_empty() {
            format!(" {} ", self.status_msg)
        } else {
            " q:quit  \u{2423}:pause  n/p:next/prev  s:shuffle  /:search ".to_string()
        };
        let hints = ratatui::widgets::Paragraph::new(hint_text).style(theme::status_bar());
        frame.render_widget(hints, status_chunks[0]);

        // Right: tabs
        let tab_index = match self.view {
            View::Collection => 0,
            View::Album => 1,
            View::Queue => 2,
        };
        let tabs = ratatui::widgets::Tabs::new(vec![
            " 1 Collection ",
            " 2 Album ",
            " 3 Queue ",
        ])
        .select(tab_index)
        .style(theme::status_bar())
        .highlight_style(theme::selected())
        .divider("\u{2502}");
        frame.render_widget(tabs, status_chunks[1]);
    }
}
