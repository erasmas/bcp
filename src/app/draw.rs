use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};
use ratatui_image::StatefulImage;

use super::{App, AppScreen, Column, LoginStep};
use crate::ui::logo::{LOGO, logo_gradient};
use crate::ui::theme;
use crate::ui::views::album::TrackColumn;
use crate::ui::views::artist_column::ArtistColumn;
use crate::ui::views::collection::AlbumColumn;
use crate::ui::views::now_playing::NowPlayingBar;
use crate::ui::views::queue::QueueColumn;
use crate::ui::views::settings::SettingsView;

impl App {
    pub fn draw(&mut self, frame: &mut Frame) {
        match self.screen {
            AppScreen::Login => self.draw_login(frame),
            AppScreen::Loading => self.draw_loading(frame),
            AppScreen::Main => self.draw_main(frame),
        }
    }

    fn draw_login(&self, frame: &mut Frame) {
        let area = frame.area();
        // Layout is symmetric around the prompt line so it sits exactly in the
        // vertical center: top Fill + logo(12) + gap(1) == status(2) + info(11) + bottom Fill.
        let chunks = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(12), // logo
            Constraint::Length(1),  // gap
            Constraint::Length(1),  // prompt (centered)
            Constraint::Length(2),  // status
            Constraint::Length(11), // info
            Constraint::Fill(1),
        ])
        .split(area);

        let gradient = logo_gradient(LOGO.len());
        let logo: Vec<Line> = LOGO
            .iter()
            .enumerate()
            .map(|(i, line)| {
                Line::from(Span::styled(
                    *line,
                    ratatui::style::Style::default()
                        .fg(gradient[i % gradient.len()])
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ))
            })
            .collect();
        let logo_widget = Paragraph::new(logo).alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(logo_widget, chunks[1]);

        let msg = match self.login_step {
            LoginStep::Prompt => "Press Enter to open Bandcamp login in your browser",
            LoginStep::WaitingForBrowser => {
                "Log in to Bandcamp in your browser, then press Enter here"
            }
            LoginStep::Extracting => "Extracting session cookie...",
        };
        let prompt = Paragraph::new(msg)
            .style(theme::normal())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(prompt, chunks[3]);

        if !self.status_msg.is_empty() {
            let status = Paragraph::new(self.status_msg.as_str())
                .style(theme::dim())
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(status, chunks[4]);
        }

        let info = "How this works:\n\
                    \n\
                    When you press Enter, bcp opens bandcamp.com in your default browser.\n\
                    Once you're logged in, bcp reads the session cookie from the browser\n\
                    profile and uses it to authenticate API requests on your behalf -\n\
                    the cookie itself stays on your machine and is never uploaded.\n\
                    \n\
                    If your default browser is Chromium-based, your OS may ask once\n\
                    to unlock the keyring where the cookie is stored. Firefox-based\n\
                    browsers store cookies on disk directly and need no prompt.";
        let info_widget = Paragraph::new(info)
            .style(theme::dim())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(info_widget, chunks[5]);
    }

    fn draw_loading(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::vertical([
            Constraint::Percentage(40),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

        let msg = Paragraph::new(self.status_msg.as_str())
            .style(theme::normal())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(msg, chunks[1]);
    }

    fn draw_main(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::vertical([
            Constraint::Percentage(30),
            Constraint::Min(10),
            Constraint::Length(1),
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
            stream_bitrate: self.stream_bitrate.as_deref(),
        };
        let np_area = chunks[0];
        self.np_rect = np_area;
        frame.render_widget(now_playing, np_area);

        // Album art
        if let Some(ref mut protocol) = self.art_protocol {
            let art_rect = NowPlayingBar::art_area(np_area);
            let art_widget = StatefulImage::default().resize(ratatui_image::Resize::Scale(None));
            frame.render_stateful_widget(art_widget, art_rect, protocol);
        }

        // Three-column layout — sliding window over four logical columns.
        // When Queue is focused: [Albums | Tracks | Queue]
        // Otherwise:             [Artists | Albums | Tracks]
        let columns = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ])
        .split(chunks[1]);

        // Sliding window: show [Albums | Tracks | Queue] while queue_visible,
        // otherwise [Artists | Albums | Tracks]. The viewport only resets when
        // focus moves all the way back to Artists.
        if self.queue_visible {
            self.artist_rect = Rect::ZERO;
            self.album_rect = columns[0];
            self.track_rect = columns[1];
            self.queue_rect = columns[2];
        } else {
            self.artist_rect = columns[0];
            self.album_rect = columns[1];
            self.track_rect = columns[2];
            self.queue_rect = Rect::ZERO;
        }

        // Column 1: Artists (hidden while queue panel is open)
        if !self.queue_visible {
            let artist_view = ArtistColumn {
                artists: &self.artist_index.artists,
                filtered_indices: &self.artist_filtered,
                focused: self.focus == Column::Artists,
            };
            frame.render_stateful_widget(artist_view, columns[0], &mut self.artist_state);
        }

        // Column 1 (queue mode) / Column 2: Albums for selected artist
        let artist_albums = self.current_artist_album_indices();
        let album_col = if self.queue_visible { columns[0] } else { columns[1] };
        let album_view = AlbumColumn {
            albums: &self.albums,
            album_indices: &artist_albums,
            filtered_indices: &self.album_filtered,
            library: &self.library,
            focused: self.focus == Column::Albums,
        };
        frame.render_stateful_widget(album_view, album_col, &mut self.album_state);

        // Column 2 (queue mode) / Column 3: Tracks for selected album
        let current = self.queue.current_item();
        let playing_album_id = current.map(|q| q.item_id);
        let playing_track_num = current.map(|q| q.track.track_num);
        let selected_album = self.selected_album_idx.and_then(|i| self.albums.get(i));
        let track_col = if self.queue_visible { columns[1] } else { columns[2] };
        let track_view = TrackColumn {
            album: selected_album,
            playing_album_id,
            playing_track_num,
            filtered_indices: &self.track_filtered,
            library: &self.library,
            focused: self.focus == Column::Tracks,
            loading: self.loading_tracks,
        };
        frame.render_stateful_widget(track_view, track_col, &mut self.track_state);

        // Column 3: Queue (always rendered while queue panel is open)
        if self.queue_visible {
            let queue_view = QueueColumn {
                items: &self.queue.items,
                current: self.queue.current,
                focused: self.focus == Column::Queue,
            };
            frame.render_stateful_widget(queue_view, columns[2], &mut self.queue_state);
        }

        // Status bar
        let status_area = chunks[2];

        if self.filter_mode {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let cursor = if (now / 500).is_multiple_of(2) {
                "\u{2502}"
            } else {
                " "
            };
            let search_text = format!(" search: {}{}", self.filter_text, cursor);
            let search_bar = Paragraph::new(search_text).style(
                ratatui::style::Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            );
            frame.render_widget(search_bar, status_area);
            return;
        }

        let status_chunks =
            Layout::horizontal([Constraint::Min(10), Constraint::Min(5)]).split(status_area);

        // Breadcrumb
        let mut crumbs: Vec<Span> = vec![Span::styled(" Artists", theme::dim())];
        if let Some(artist) = self.selected_artist_name() {
            crumbs.push(Span::styled(" > ", theme::dim()));
            crumbs.push(Span::styled(artist, theme::dim()));
            if let Some(album) = self.selected_album() {
                crumbs.push(Span::styled(" > ", theme::dim()));
                crumbs.push(Span::styled(album.album_title.as_str(), theme::dim()));
            }
        }
        let breadcrumb = Paragraph::new(Line::from(crumbs));
        frame.render_widget(breadcrumb, status_chunks[0]);

        let right_text = if self.status_msg.is_empty() {
            " ? help ".to_string()
        } else {
            format!(" {} ", self.status_msg)
        };
        let status = Paragraph::new(right_text)
            .style(theme::dim())
            .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(status, status_chunks[1]);

        // Settings overlay
        if self.show_settings {
            let overlay_area = centered_rect(80, 80, chunks[1]);
            frame.render_widget(Clear, overlay_area);
            let username = self
                .auth
                .as_ref()
                .and_then(|a| a.username.as_deref())
                .unwrap_or("not logged in");
            let downloaded_count = self
                .library
                .albums
                .values()
                .filter(|a| a.status == crate::library::AlbumDownloadStatus::Complete)
                .count();
            let view = SettingsView {
                username,
                album_count: self.albums.len(),
                downloaded_count,
            };
            frame.render_widget(view, overlay_area);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}
