use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::config;
use crate::ui::theme;

pub struct SettingsView<'a> {
    pub username: &'a str,
    pub album_count: usize,
    pub downloaded_count: usize,
}

impl<'a> Widget for SettingsView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let config_dir = config::config_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let cache_dir = config::cache_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let library_dir = config::library_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Account:      ", theme::dim()),
                Span::styled(self.username, theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("  Collection:   ", theme::dim()),
                Span::styled(format!("{} albums", self.album_count), theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("  Downloaded:   ", theme::dim()),
                Span::styled(format!("{} albums", self.downloaded_count), theme::normal()),
            ]),
            Line::from(""),
            Line::from(Span::styled("  Paths", theme::selected())),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Config:       ", theme::dim()),
                Span::styled(&config_dir, theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("  Cache:        ", theme::dim()),
                Span::styled(&cache_dir, theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("  Library:      ", theme::dim()),
                Span::styled(&library_dir, theme::normal()),
            ]),
            Line::from(""),
            Line::from(Span::styled("  Keybindings", theme::selected())),
            Line::from(""),
            Line::from(vec![
                Span::styled("  h/l ", theme::normal()),
                Span::styled("left/right  ", theme::dim()),
                Span::styled("j/k ", theme::normal()),
                Span::styled("up/down  ", theme::dim()),
                Span::styled("Enter ", theme::normal()),
                Span::styled("open/play  ", theme::dim()),
            ]),
            Line::from(vec![
                Span::styled("  Space ", theme::normal()),
                Span::styled("pause  ", theme::dim()),
                Span::styled("n/p ", theme::normal()),
                Span::styled("next/prev  ", theme::dim()),
                Span::styled("/ ", theme::normal()),
                Span::styled("search  ", theme::dim()),
            ]),
            Line::from(vec![
                Span::styled("  d ", theme::normal()),
                Span::styled("download  ", theme::dim()),
                Span::styled("D ", theme::normal()),
                Span::styled("download all  ", theme::dim()),
                Span::styled("r ", theme::normal()),
                Span::styled("refresh  ", theme::dim()),
            ]),
            Line::from(vec![
                Span::styled("  ? ", theme::normal()),
                Span::styled("info  ", theme::dim()),
                Span::styled("q ", theme::normal()),
                Span::styled("quit", theme::dim()),
            ]),
        ];

        let block = Block::default()
            .title(" Info ")
            .title_style(theme::title())
            .borders(Borders::ALL)
            .border_style(theme::dim());

        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(area, buf);
    }
}
