use anyhow::Result;

use super::{App, AppScreen, Column, LoginStep};

/// Parse the bitrate from the first MP3 frame header.
/// Returns e.g. "128 kbps" for CBR or "VBR" for variable-bitrate files.
fn parse_mp3_bitrate(data: &[u8]) -> Option<String> {
    // Skip ID3v2 tag if present
    let start = if data.starts_with(b"ID3") && data.len() >= 10 {
        let size = ((data[6] as usize) << 21)
            | ((data[7] as usize) << 14)
            | ((data[8] as usize) << 7)
            | (data[9] as usize);
        10 + size
    } else {
        0
    };

    // Find first frame sync (0xFF followed by 0xEx or 0xFx)
    let frame_start = data[start..]
        .windows(2)
        .position(|w| w[0] == 0xFF && (w[1] & 0xE0) == 0xE0)?
        + start;

    if frame_start + 4 > data.len() {
        return None;
    }

    let h1 = data[frame_start + 1];
    let h2 = data[frame_start + 2];
    let h3 = data[frame_start + 3];

    // Only handle MPEG1 (bits 4-3 of h1 = 11), Layer 3 (bits 2-1 of h1 = 01)
    if (h1 & 0x18) != 0x18 || (h1 & 0x06) != 0x02 {
        return None;
    }

    let bitrate_idx = (h2 >> 4) as usize;
    let bitrates = [
        0u32, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    let bitrate = *bitrates.get(bitrate_idx)?;
    if bitrate == 0 {
        return None;
    }

    // Check for Xing VBR header: located after the frame header + side info
    // Side info: 32 bytes for stereo (channel_mode != 3), 17 bytes for mono
    let is_mono = (h3 >> 6) == 3;
    let side_info_size = if is_mono { 17 } else { 32 };
    let xing_offset = frame_start + 4 + side_info_size;
    if xing_offset + 4 <= data.len() && &data[xing_offset..xing_offset + 4] == b"Xing" {
        return Some("VBR".to_string());
    }

    Some(format!("{} kbps", bitrate))
}
use crate::bandcamp::client::{AlbumDetail, BandcampClient};
use crate::bandcamp::models::AuthData;
use crate::player::queue::QueueItem;
use crate::{auth, cache, library};

impl Message {
    /// Returns (key_label, description) if this is a user-facing keybinding.
    /// Exhaustive match - adding a new Message variant without handling it
    /// will cause a compile error.
    pub fn binding(&self) -> Option<(&'static str, &'static str)> {
        match self {
            // Navigation
            Self::Quit => Some(("q", "quit")),
            Self::FocusLeft => Some(("h / \u{2190}", "left")),
            Self::FocusRight => Some(("l / \u{2192}", "right")),
            Self::FocusColumn(_) | Self::SelectAt(_, _) | Self::ScrollColumn(_, _) => None,
            Self::MoveUp => Some(("k / \u{2191}", "up")),
            Self::MoveDown => Some(("j / \u{2193}", "down")),
            Self::MoveToTop => Some(("g", "top")),
            Self::MoveToBottom => Some(("G", "bottom")),
            Self::PageDown => Some(("PgDn / ^F", "page down")),
            Self::PageUp => Some(("PgUp / ^B", "page up")),
            Self::HalfPageDown => Some(("^D", "half page down")),
            Self::HalfPageUp => Some(("^U", "half page up")),
            Self::Enter => Some(("Enter", "open/play")),
            // Playback
            Self::TogglePause => Some(("Space", "pause")),
            Self::NextTrack => Some(("n", "next track")),
            Self::PrevTrack => Some(("p", "prev track")),
            Self::SeekBackward => Some(("[", "seek -10s")),
            Self::SeekForward => Some(("]", "seek +10s")),
            Self::ScrollMetaDown => Some(("J", "scroll meta down")),
            Self::ScrollMetaUp => Some(("K", "scroll meta up")),
            // Filter
            Self::StartFilter => Some(("/", "search")),
            Self::CancelFilter => Some(("Esc", "back/clear")),
            Self::FilterChar(_) | Self::FilterBackspace | Self::ConfirmFilter => None,
            // Downloads
            Self::Download => Some(("d", "download")),
            Self::DownloadAll => Some(("D", "download all")),
            // UI
            Self::ToggleSettings => Some(("?", "info")),
            Self::Refresh => Some(("r", "refresh")),
            Self::Yank => Some(("y", "yank link")),
            // Internal (login, async results)
            Self::OpenLogin | Self::ExtractCookie => None,
            Self::AlbumDetailLoaded { .. } | Self::AlbumDetailFailed(_) => None,
            Self::PrefetchResult { .. } | Self::PrefetchDone => None,
            Self::Mp3Ready(_) | Self::Mp3Failed(_) => None,
            Self::ArtReady(_) => None,
            Self::TrackFinished | Self::PlaybackError(_) => None,
            Self::DownloadTrackDone { .. }
            | Self::DownloadAlbumDone { .. }
            | Self::DownloadError { .. } => None,
        }
    }

    /// All user-facing keybindings in display order.
    pub fn all_keybindings() -> Vec<(&'static str, &'static str)> {
        [
            Message::FocusLeft,
            Message::FocusRight,
            Message::MoveUp,
            Message::MoveDown,
            Message::MoveToTop,
            Message::MoveToBottom,
            Message::PageDown,
            Message::PageUp,
            Message::HalfPageDown,
            Message::HalfPageUp,
            Message::Enter,
            Message::CancelFilter,
            Message::StartFilter,
            Message::TogglePause,
            Message::NextTrack,
            Message::PrevTrack,
            Message::SeekBackward,
            Message::SeekForward,
            Message::ScrollMetaDown,
            Message::ScrollMetaUp,
            Message::Download,
            Message::DownloadAll,
            Message::Refresh,
            Message::Yank,
            Message::ToggleSettings,
            Message::Quit,
        ]
        .into_iter()
        .filter_map(|m| m.binding())
        .collect()
    }
}

pub enum Message {
    // Navigation
    Quit,
    FocusLeft,
    FocusRight,
    FocusColumn(Column),
    SelectAt(Column, usize),
    ScrollColumn(Column, isize),
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,
    PageDown,
    PageUp,
    HalfPageDown,
    HalfPageUp,
    Enter,

    // Playback
    TogglePause,
    NextTrack,
    PrevTrack,
    SeekBackward,
    SeekForward,
    ScrollMetaDown,
    ScrollMetaUp,

    // Filter
    StartFilter,
    FilterChar(char),
    FilterBackspace,
    CancelFilter,
    ConfirmFilter,

    // Downloads
    Download,
    DownloadAll,

    // UI
    ToggleSettings,
    Refresh,
    Yank,
    // Login
    OpenLogin,
    ExtractCookie,

    // Async results
    AlbumDetailLoaded { idx: usize, detail: AlbumDetail },
    AlbumDetailFailed(String),
    PrefetchResult { idx: usize, detail: AlbumDetail },
    PrefetchDone,
    Mp3Ready(Vec<u8>),
    Mp3Failed(String),
    ArtReady(image::DynamicImage),
    TrackFinished,
    PlaybackError(String),
    DownloadTrackDone { item_id: u64, track_num: u32 },
    DownloadAlbumDone { item_id: u64 },
    DownloadError { item_id: u64, msg: String },
}

impl App {
    pub async fn update(&mut self, msg: Message) -> Result<()> {
        match msg {
            // -- Navigation --
            Message::Quit => {
                self.persist_state();
                self.should_quit = true;
            }

            Message::FocusLeft => {
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

            Message::FocusRight => {
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

            Message::FocusColumn(col) => {
                if self.focus != col {
                    if !self.filter_text.is_empty() {
                        self.filter_text.clear();
                    }
                    // Only allow focusing columns that have content / are reachable.
                    match col {
                        Column::Artists => {
                            self.focus = Column::Artists;
                            self.recompute_active_filter();
                        }
                        Column::Albums => {
                            if !self.album_filtered.is_empty() {
                                self.focus = Column::Albums;
                                self.recompute_active_filter();
                            }
                        }
                        Column::Tracks => {
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
                    }
                }
            }

            Message::ScrollColumn(col, delta) => {
                let (len, rect, state) = match col {
                    Column::Artists => (
                        self.artist_filtered.len(),
                        self.artist_rect,
                        &mut self.artist_state,
                    ),
                    Column::Albums => (
                        self.album_filtered.len(),
                        self.album_rect,
                        &mut self.album_state,
                    ),
                    Column::Tracks => (
                        self.track_filtered.len(),
                        self.track_rect,
                        &mut self.track_state,
                    ),
                };
                if len == 0 || rect.height < 3 {
                    return Ok(());
                }
                let visible = (rect.height - 2) as usize;
                let max_offset = len.saturating_sub(visible);
                let cur_offset = state.offset() as isize;
                let new_offset = (cur_offset + delta).clamp(0, max_offset as isize) as usize;
                *state.offset_mut() = new_offset;
            }

            Message::SelectAt(col, idx) => {
                let len = match col {
                    Column::Artists => self.artist_filtered.len(),
                    Column::Albums => self.album_filtered.len(),
                    Column::Tracks => self.track_filtered.len(),
                };
                if idx < len {
                    match col {
                        Column::Artists => self.artist_state.select(Some(idx)),
                        Column::Albums => self.album_state.select(Some(idx)),
                        Column::Tracks => self.track_state.select(Some(idx)),
                    }
                    let prev_focus = self.focus;
                    self.focus = col;
                    if prev_focus != col {
                        self.recompute_active_filter();
                    }
                    self.on_selection_moved();
                }
            }

            Message::MoveDown => {
                self.move_selection(1);
                self.on_selection_moved();
            }

            Message::MoveUp => {
                self.move_selection(-1);
                self.on_selection_moved();
            }

            Message::MoveToTop => {
                match self.focus {
                    Column::Artists => self.artist_state.select(Some(0)),
                    Column::Albums => self.album_state.select(Some(0)),
                    Column::Tracks => self.track_state.select(Some(0)),
                }
                self.on_selection_moved();
            }

            Message::MoveToBottom => {
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
                self.on_selection_moved();
            }

            Message::PageDown => {
                let page = self.focus_page_size();
                self.move_selection(page as i32);
                self.on_selection_moved();
            }

            Message::PageUp => {
                let page = self.focus_page_size();
                self.move_selection(-(page as i32));
                self.on_selection_moved();
            }

            Message::HalfPageDown => {
                let half = (self.focus_page_size() / 2).max(1);
                self.move_selection(half as i32);
                self.on_selection_moved();
            }

            Message::HalfPageUp => {
                let half = (self.focus_page_size() / 2).max(1);
                self.move_selection(-(half as i32));
                self.on_selection_moved();
            }

            Message::Enter => match self.focus {
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
                    self.play_selected_track();
                }
            },

            // -- Playback --
            Message::TogglePause => {
                self.toggle_pause()?;
            }
            Message::NextTrack => self.play_next(),
            Message::PrevTrack => self.play_prev(),
            Message::SeekBackward => self.seek_by(-10.0),
            Message::SeekForward => self.seek_by(10.0),

            Message::ScrollMetaDown => {
                self.meta_scroll += 1;
            }
            Message::ScrollMetaUp => {
                self.meta_scroll = self.meta_scroll.saturating_sub(1);
            }

            // -- Filter --
            Message::StartFilter => {
                self.filter_mode = true;
                self.filter_text.clear();
                self.recompute_active_filter();
            }
            Message::FilterChar(c) => {
                self.filter_text.push(c);
                self.recompute_active_filter();
                match self.focus {
                    Column::Artists => self.artist_state.select(Some(0)),
                    Column::Albums => self.album_state.select(Some(0)),
                    Column::Tracks => self.track_state.select(Some(0)),
                }
            }
            Message::FilterBackspace => {
                self.filter_text.pop();
                self.recompute_active_filter();
            }
            Message::CancelFilter => {
                self.filter_mode = false;
                self.filter_text.clear();
                self.recompute_active_filter();
            }
            Message::ConfirmFilter => {
                self.filter_mode = false;
            }

            // -- Downloads --
            Message::Download => self.download_context().await,
            Message::DownloadAll => self.download_all_albums().await,

            // -- UI --
            Message::ToggleSettings => {
                self.show_settings = !self.show_settings;
            }
            Message::Refresh => {
                cache::invalidate_cache()?;
                self.albums.clear();
                self.screen = AppScreen::Loading;
                self.status_msg = "Refreshing collection...".to_string();
            }
            Message::Yank => {
                self.yank_current_link();
            }

            // -- Login --
            Message::OpenLogin => {
                auth::open_login_page()?;
                self.login_step = LoginStep::WaitingForBrowser;
                self.status_msg = "Browser opened - log in, then press Enter here".to_string();
            }
            Message::ExtractCookie => {
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
            }

            // -- Async results --
            Message::AlbumDetailLoaded { idx, detail } => {
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
                let _ = library::save_album_metadata(&album.artist_name, &album.album_title, &meta);
                self.recompute_track_filter();
                if !self.track_filtered.is_empty() && self.track_state.selected().is_none() {
                    self.track_state.select(Some(0));
                }
                self.status_msg = String::new();
                self.loading_tracks = false;
                self.dirty = true;
            }
            Message::AlbumDetailFailed(e) => {
                self.status_msg = format!("Failed to load album: {}", e);
                self.loading_tracks = false;
                self.dirty = true;
            }

            Message::PrefetchResult { idx, detail } => {
                self.albums[idx].tracks = detail.tracks;
                self.albums[idx].about = detail.about;
                self.albums[idx].credits = detail.credits;
                self.albums[idx].release_date = detail.release_date;
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
                let _ = library::save_album_metadata(&album.artist_name, &album.album_title, &meta);
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
                let _ = self.library.save();
                self.recompute_track_filter();
                self.dirty = true;
            }
            Message::PrefetchDone => {
                self.detail_rx = None;
                let _ = cache::save_collection_cache(&self.albums);
                self.status_msg = String::new();
                self.dirty = true;
            }

            Message::Mp3Ready(data) => {
                if let Some(ref engine) = self.engine {
                    self.stream_bitrate = parse_mp3_bitrate(&data);
                    engine.play(data)?;
                    self.is_paused = false;
                    self.play_started = Some(std::time::Instant::now());
                    self.pause_accumulated = 0.0;
                    self.elapsed = 0.0;
                    self.meta_scroll = 0;
                    self.status_msg = String::new();
                    self.dirty = true;
                }
            }
            Message::Mp3Failed(e) => {
                self.status_msg = format!("Stream error: {}", e);
                self.dirty = true;
            }

            Message::ArtReady(img) => {
                if let Some(ref mut picker) = self.art_picker {
                    self.art_protocol = Some(picker.new_resize_protocol(img));
                    self.dirty = true;
                }
            }

            Message::TrackFinished => self.play_next(),
            Message::PlaybackError(e) => {
                self.status_msg = format!("Playback error: {}", e);
                self.dirty = true;
            }

            Message::DownloadTrackDone { item_id, track_num } => {
                if let Some(album) = self.library.albums.get_mut(&item_id)
                    && let Some(track) = album.tracks.iter_mut().find(|t| t.track_num == track_num)
                {
                    track.downloaded = true;
                }
                self.dirty = true;
            }
            Message::DownloadAlbumDone { item_id } => {
                if let Some(album) = self.library.albums.get_mut(&item_id) {
                    album.status = library::AlbumDownloadStatus::Complete;
                    self.status_msg =
                        format!("Downloaded: {} - {}", album.artist_name, album.album_title);
                }
                let _ = self.library.save();
                self.dirty = true;
            }
            Message::DownloadError { item_id, msg } => {
                self.status_msg = format!("Download error ({}): {}", item_id, msg);
                self.dirty = true;
            }
        }
        Ok(())
    }

    // -- Helper methods used by update --

    fn focus_page_size(&self) -> usize {
        let rect = match self.focus {
            Column::Artists => self.artist_rect,
            Column::Albums => self.album_rect,
            Column::Tracks => self.track_rect,
        };
        if rect.height < 3 {
            1
        } else {
            (rect.height - 2) as usize
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

    /// Adjust the cached `ListState::offset` of `col` so the selected item is
    /// visible. Needed because we render lists with the selection-cleared
    /// trick (so ratatui no longer auto-scrolls into view).
    pub(crate) fn ensure_visible(&mut self, col: Column) {
        let (state, rect, len) = match col {
            Column::Artists => (
                &mut self.artist_state,
                self.artist_rect,
                self.artist_filtered.len(),
            ),
            Column::Albums => (
                &mut self.album_state,
                self.album_rect,
                self.album_filtered.len(),
            ),
            Column::Tracks => (
                &mut self.track_state,
                self.track_rect,
                self.track_filtered.len(),
            ),
        };
        if rect.height < 3 || len == 0 {
            return;
        }
        let visible = (rect.height - 2) as usize;
        let max_off = len.saturating_sub(visible);
        let off = state.offset().min(max_off);
        let new_off = if let Some(sel) = state.selected() {
            if sel < off {
                sel
            } else if sel >= off + visible {
                sel + 1 - visible
            } else {
                off
            }
        } else {
            off
        };
        if new_off != state.offset() {
            *state.offset_mut() = new_off;
        }
    }

    fn on_selection_moved(&mut self) {
        self.ensure_visible(self.focus);
        match self.focus {
            Column::Artists => self.on_artist_changed(),
            Column::Albums => self.on_album_changed(),
            Column::Tracks => {}
        }
    }

    fn play_selected_track(&mut self) {
        let Some(album_idx) = self.selected_album_idx else {
            return;
        };
        let Some(selected) = self.track_state.selected() else {
            return;
        };
        let Some(&track_idx) = self.track_filtered.get(selected) else {
            return;
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

    fn start_loading_album_details(&mut self, idx: usize) {
        if !self.albums[idx].tracks.is_empty() {
            return;
        }

        // Try local metadata.json first
        if let Some(meta) = library::load_album_metadata(
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
