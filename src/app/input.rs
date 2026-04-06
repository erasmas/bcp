use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, AppScreen, Column, LoginStep};
use crate::auth;
use crate::bandcamp::client::BandcampClient;
use crate::bandcamp::models::AuthData;
use crate::cache;
use crate::player::queue::QueueItem;

impl App {
    pub(crate) async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return Ok(());
        }

        match self.screen {
            AppScreen::Login => self.handle_login_key(key).await?,
            AppScreen::Loading => {}
            AppScreen::Main => {
                if self.show_settings {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('?') => self.show_settings = false,
                        KeyCode::Char('q') => self.should_quit = true,
                        _ => {}
                    }
                    return Ok(());
                }
                if self.filter_mode {
                    let is_nav = matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Enter);
                    if !is_nav {
                        self.handle_filter_key(key)?;
                        return Ok(());
                    }
                    if key.code == KeyCode::Enter {
                        self.handle_filter_key(key)?;
                    }
                }
                self.handle_main_key(key).await?;
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
                    self.status_msg = "Browser opened - log in, then press Enter here".to_string();
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
                                "Could not find cookie - make sure you're logged in, then press Enter"
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
            KeyCode::Esc => {
                if !self.filter_text.is_empty() {
                    self.filter_text.clear();
                    self.recompute_active_filter();
                } else {
                    self.handle_left();
                }
            }
            KeyCode::Char('h') | KeyCode::Left => self.handle_left(),
            KeyCode::Char('l') | KeyCode::Right => self.handle_right(),
            KeyCode::Char('?') => self.show_settings = !self.show_settings,
            KeyCode::Char('d') => self.download_context().await,
            KeyCode::Char('D') => self.download_all_albums().await,
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_selection(1);
                self.on_selection_moved();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_selection(-1);
                self.on_selection_moved();
            }
            KeyCode::Char('g') => {
                self.move_to_top();
                self.on_selection_moved();
            }
            KeyCode::Char('G') => {
                self.move_to_bottom();
                self.on_selection_moved();
            }
            KeyCode::Enter => self.handle_enter().await?,
            KeyCode::Char(' ') => self.toggle_pause()?,
            KeyCode::Char('n') => self.play_next(),
            KeyCode::Char('p') => self.play_prev(),
            KeyCode::Char('J') => {
                let offset = self.meta_scroll.unwrap_or(0);
                self.meta_scroll = Some(offset + 1);
            }
            KeyCode::Char('K') => {
                let offset = self.meta_scroll.unwrap_or(0);
                self.meta_scroll = Some(offset.saturating_sub(1));
            }
            KeyCode::Tab => {
                if self.meta_scroll.is_some() {
                    self.meta_scroll = None;
                } else {
                    self.meta_scroll = Some(0);
                }
            }
            KeyCode::Char('r') => {
                cache::invalidate_cache()?;
                self.albums.clear();
                self.screen = AppScreen::Loading;
                self.status_msg = "Refreshing collection...".to_string();
            }
            KeyCode::Char('/') => {
                self.filter_mode = true;
                self.filter_text.clear();
                self.recompute_active_filter();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.filter_text.clear();
                self.recompute_active_filter();
            }
            KeyCode::Enter => {
                self.filter_mode = false;
            }
            KeyCode::Backspace => {
                self.filter_text.pop();
                self.recompute_active_filter();
            }
            KeyCode::Char(c) => {
                self.filter_text.push(c);
                self.recompute_active_filter();
                match self.focus {
                    Column::Artists => self.artist_state.select(Some(0)),
                    Column::Albums => self.album_state.select(Some(0)),
                    Column::Tracks => self.track_state.select(Some(0)),
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_left(&mut self) {
        if !self.filter_text.is_empty() {
            self.filter_text.clear();
        }
        match self.focus {
            Column::Tracks => {
                self.focus = Column::Albums;
                self.recompute_active_filter();
            }
            Column::Albums => {
                self.focus = Column::Artists;
                self.recompute_active_filter();
            }
            Column::Artists => {}
        }
    }

    fn handle_right(&mut self) {
        if !self.filter_text.is_empty() {
            self.filter_text.clear();
        }
        match self.focus {
            Column::Artists => {
                if !self.album_filtered.is_empty() {
                    self.focus = Column::Albums;
                    self.recompute_active_filter();
                }
            }
            Column::Albums => {
                if let Some(album_idx) = self.selected_album_idx {
                    if self.albums[album_idx].tracks.is_empty() {
                        self.start_loading_album_details(album_idx);
                        self.recompute_track_filter();
                        if !self.track_filtered.is_empty() {
                            self.track_state.select(Some(0));
                        }
                    }
                    self.focus = Column::Tracks;
                    self.recompute_active_filter();
                }
            }
            Column::Tracks => {}
        }
    }

    fn on_selection_moved(&mut self) {
        match self.focus {
            Column::Artists => self.on_artist_changed(),
            Column::Albums => self.on_album_changed(),
            Column::Tracks => {}
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let (state, len) = match self.focus {
            Column::Artists => (&mut self.artist_state, self.artist_filtered.len()),
            Column::Albums => (&mut self.album_state, self.album_filtered.len()),
            Column::Tracks => (&mut self.track_state, self.track_filtered.len()),
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
        match self.focus {
            Column::Artists => self.artist_state.select(Some(0)),
            Column::Albums => self.album_state.select(Some(0)),
            Column::Tracks => self.track_state.select(Some(0)),
        }
    }

    fn move_to_bottom(&mut self) {
        let len = match self.focus {
            Column::Artists => self.artist_filtered.len(),
            Column::Albums => self.album_filtered.len(),
            Column::Tracks => self.track_filtered.len(),
        };
        if len > 0 {
            match self.focus {
                Column::Artists => self.artist_state.select(Some(len - 1)),
                Column::Albums => self.album_state.select(Some(len - 1)),
                Column::Tracks => self.track_state.select(Some(len - 1)),
            }
        }
    }

    async fn handle_enter(&mut self) -> Result<()> {
        match self.focus {
            Column::Artists => {
                if !self.album_filtered.is_empty() {
                    if !self.filter_text.is_empty() {
                        self.filter_text.clear();
                    }
                    self.focus = Column::Albums;
                    self.recompute_active_filter();
                }
            }
            Column::Albums => {
                if let Some(album_idx) = self.selected_album_idx {
                    if self.albums[album_idx].tracks.is_empty() {
                        self.start_loading_album_details(album_idx);
                        self.recompute_track_filter();
                        if !self.track_filtered.is_empty() {
                            self.track_state.select(Some(0));
                        }
                    }
                    if !self.filter_text.is_empty() {
                        self.filter_text.clear();
                    }
                    self.focus = Column::Tracks;
                    self.recompute_active_filter();
                    self.status_msg = String::new();
                }
            }
            Column::Tracks => {
                let Some(album_idx) = self.selected_album_idx else {
                    return Ok(());
                };
                let Some(selected) = self.track_state.selected() else {
                    return Ok(());
                };
                let Some(&track_idx) = self.track_filtered.get(selected) else {
                    return Ok(());
                };

                let album = &self.albums[album_idx];

                let items: Vec<QueueItem> = album
                    .tracks
                    .iter()
                    .map(|t| QueueItem {
                        track: t.clone(),
                        item_id: album.item_id,
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
        }
        Ok(())
    }

    fn start_loading_album_details(&mut self, idx: usize) {
        if !self.albums[idx].tracks.is_empty() {
            return;
        }

        // Try local metadata.json first
        if let Some(meta) = crate::library::load_album_metadata(
            &self.albums[idx].artist_name,
            &self.albums[idx].album_title,
        ) {
            self.albums[idx].tracks = meta
                .tracks
                .iter()
                .map(|t| crate::bandcamp::models::Track {
                    title: t.title.clone(),
                    track_num: t.track_num,
                    duration: t.duration,
                    stream_url: t.stream_url.clone(),
                })
                .collect();
            self.albums[idx].about = meta.about;
            self.albums[idx].credits = meta.credits;
            self.albums[idx].release_date = meta.release_date;
            self.recompute_track_filter();
            if !self.track_filtered.is_empty() {
                self.track_state.select(Some(0));
            }
            self.dirty = true;
            return;
        }

        // Fall back to library index
        let item_id = self.albums[idx].item_id;
        if let Some(dl_album) = self.library.albums.get(&item_id) {
            self.albums[idx].tracks = dl_album
                .tracks
                .iter()
                .map(|t| crate::bandcamp::models::Track {
                    title: t.title.clone(),
                    track_num: t.track_num,
                    duration: t.duration,
                    stream_url: None,
                })
                .collect();
            self.recompute_track_filter();
            if !self.track_filtered.is_empty() {
                self.track_state.select(Some(0));
            }
            self.dirty = true;
            return;
        }

        if self.loading_tracks {
            return;
        }
        let Some(ref client) = self.client else {
            return;
        };
        self.loading_tracks = true;
        self.dirty = true;

        let url = self.albums[idx].item_url.clone();
        let http = client.clone_http();
        let cookie = client.cookie_header();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tracks_rx = Some((idx, rx));

        tokio::spawn(async move {
            let result = crate::bandcamp::client::BandcampClient::fetch_album_details_static(
                &http, &cookie, &url,
            )
            .await;
            let _ = tx.send(result);
        });
    }
}
