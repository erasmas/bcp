use ratatui::{
    layout::{Constraint, Layout},
    Frame,
};
use ratatui_image::StatefulImage;

use super::{App, AppScreen, LoginStep, View};
use crate::ui::theme;
use crate::ui::views::album::AlbumView;
use crate::ui::views::collection::CollectionView;
use crate::ui::views::settings::SettingsView;
use crate::ui::views::downloaded::DownloadedView;
use crate::ui::views::now_playing::NowPlayingBar;

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
            Constraint::Length(np_height),
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
        };
        let np_area = chunks[0];
        frame.render_widget(now_playing, np_area);

        // Album art
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
                    filter: &self.collection_filter,
                };
                frame.render_stateful_widget(view, chunks[1], &mut self.collection_state);
            }
            View::Album => {
                if let Some(idx) = self.selected_album_idx {
                    if let Some(album) = self.albums.get(idx) {
                        let current = self.queue.current_item();
                        let view = AlbumView {
                            album,
                            playing_album_id: current.map(|q| q.item_id),
                            playing_track_num: current.map(|q| q.track.track_num),
                            filter: &self.album_filter,
                        };
                        frame.render_stateful_widget(view, chunks[1], &mut self.album_state);
                    }
                }
            }
            View::Downloaded => {
                let view = DownloadedView {
                    albums: &self.albums,
                    library: &self.library,
                };
                frame.render_stateful_widget(view, chunks[1], &mut self.downloaded_state);
            }
            View::Settings => {
                let username = self.auth.as_ref()
                    .and_then(|a| a.username.as_deref())
                    .unwrap_or("not logged in");
                let downloaded_count = self.library.albums.values()
                    .filter(|a| a.status == crate::library::AlbumDownloadStatus::Complete)
                    .count();
                let view = SettingsView {
                    username,
                    album_count: self.albums.len(),
                    downloaded_count,
                };
                frame.render_widget(view, chunks[1]);
            }
        }

        // Status bar
        let status_area = chunks[2];

        if self.filter_mode {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let cursor = if (now / 500) % 2 == 0 { "\u{2502}" } else { " " };
            let search_text = format!(" search: {}{}", self.active_filter(), cursor);
            let search_bar = ratatui::widgets::Paragraph::new(search_text)
                .style(ratatui::style::Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(ratatui::style::Modifier::BOLD));
            frame.render_widget(search_bar, status_area);
            return;
        }

        let status_chunks = Layout::horizontal([
            Constraint::Length(62),
            Constraint::Min(10),
        ])
        .split(status_area);

        let tab_index = match self.view {
            View::Collection => 0,
            View::Album => 1,
            View::Downloaded => 2,
            View::Settings => 3,
        };
        let tabs = ratatui::widgets::Tabs::new(vec![
            "[1] Collection",
            "[2] Album",
            "[3] Downloaded",
            "[4] Info",
        ])
        .select(tab_index)
        .style(theme::dim())
        .highlight_style(theme::selected())
        .divider(" ");
        frame.render_widget(tabs, status_chunks[0]);

        let hint_text = if !self.status_msg.is_empty() {
            format!(" {} ", self.status_msg)
        } else {
            " quit(q)  pause(\u{2423})  next(n)  prev(p)  search(/) ".to_string()
        };
        let hints = ratatui::widgets::Paragraph::new(hint_text)
            .style(theme::dim())
            .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(hints, status_chunks[1]);
    }
}
