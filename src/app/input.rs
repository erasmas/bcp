use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, AppScreen, LoginStep, View};
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
                if self.filter_mode {
                    self.handle_filter_key(key)?;
                    if key.code != KeyCode::Enter {
                        return Ok(());
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
            KeyCode::Esc => {
                if !self.active_filter().is_empty() {
                    self.active_filter_mut().clear();
                } else {
                    match self.view {
                        View::Album | View::Downloaded => self.view = View::Collection,
                        _ => {}
                    }
                }
            }
            KeyCode::Char('1') => self.view = View::Collection,
            KeyCode::Char('2') => {
                if self.selected_album_idx.is_some() {
                    self.view = View::Album;
                }
            }
            KeyCode::Char('3') => {
                self.view = View::Downloaded;
                if self.downloaded_state.selected().is_none() && !self.albums.is_empty() {
                    self.downloaded_state.select(Some(0));
                }
            }
            KeyCode::Char('d') => self.download_selected_album().await,
            KeyCode::Char('D') => self.download_all_albums().await,
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('g') => self.move_to_top(),
            KeyCode::Char('G') => self.move_to_bottom(),
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
                if self.view == View::Collection {
                    cache::invalidate_cache()?;
                    self.albums.clear();
                    self.screen = AppScreen::Loading;
                    self.status_msg = "Refreshing collection...".to_string();
                }
            }
            KeyCode::Char('/') => {
                self.filter_mode = true;
                self.active_filter_mut().clear();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.active_filter_mut().clear();
            }
            KeyCode::Enter => {
                self.filter_mode = false;
            }
            KeyCode::Backspace => {
                self.active_filter_mut().pop();
            }
            KeyCode::Char(c) => {
                self.active_filter_mut().push(c);
                match self.view {
                    View::Collection => self.collection_state.select(Some(0)),
                    View::Album => self.album_state.select(Some(0)),
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn move_selection(&mut self, delta: i32) {
        let (state, len) = match self.view {
            View::Collection => {
                let filtered_len = if self.collection_filter.is_empty() {
                    self.albums.len()
                } else {
                    let q = self.collection_filter.to_lowercase();
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
                    .map(|a| {
                        if self.album_filter.is_empty() {
                            a.tracks.len()
                        } else {
                            let q = self.album_filter.to_lowercase();
                            a.tracks.iter().filter(|t| t.title.to_lowercase().contains(&q)).count()
                        }
                    })
                    .unwrap_or(0);
                (&mut self.album_state, len)
            }
            View::Downloaded => (&mut self.downloaded_state, self.albums.len()),
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
            View::Downloaded => self.downloaded_state.select(Some(0)),
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
            View::Downloaded => self.albums.len(),
        };
        if len > 0 {
            match self.view {
                View::Collection => self.collection_state.select(Some(len - 1)),
                View::Album => self.album_state.select(Some(len - 1)),
                View::Downloaded => self.downloaded_state.select(Some(len - 1)),
            }
        }
    }

    async fn handle_enter(&mut self) -> Result<()> {
        match self.view {
            View::Collection => {
                let Some(selected) = self.collection_state.selected() else {
                    return Ok(());
                };

                let actual_idx = if self.collection_filter.is_empty() {
                    selected
                } else {
                    let q = self.collection_filter.to_lowercase();
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

                self.load_album_details(actual_idx).await?;
                self.selected_album_idx = Some(actual_idx);
                self.album_state.select(Some(0));
                self.view = View::Album;
                self.status_msg = String::new();
            }
            View::Album => {
                let Some(album_idx) = self.selected_album_idx else {
                    return Ok(());
                };
                let Some(selected) = self.album_state.selected() else {
                    return Ok(());
                };

                let album = &self.albums[album_idx];

                let track_idx = if self.album_filter.is_empty() {
                    selected
                } else {
                    let q = self.album_filter.to_lowercase();
                    let filtered: Vec<usize> = album
                        .tracks
                        .iter()
                        .enumerate()
                        .filter(|(_, t)| t.title.to_lowercase().contains(&q))
                        .map(|(i, _)| i)
                        .collect();
                    match filtered.get(selected) {
                        Some(&idx) => idx,
                        None => return Ok(()),
                    }
                };

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
            View::Downloaded => {
                let Some(selected) = self.downloaded_state.selected() else {
                    return Ok(());
                };
                if selected >= self.albums.len() {
                    return Ok(());
                }

                self.load_album_details(selected).await?;
                self.selected_album_idx = Some(selected);
                self.album_state.select(Some(0));
                self.view = View::Album;
                self.status_msg = String::new();
            }
        }
        Ok(())
    }

    async fn load_album_details(&mut self, idx: usize) -> Result<()> {
        if !self.albums[idx].tracks.is_empty() {
            return Ok(());
        }
        self.status_msg = "Loading album...".to_string();
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
                    self.status_msg = format!("Failed to load album: {}", e);
                    anyhow::bail!("Failed to load album");
                }
            }
        }
        Ok(())
    }
}
