use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::app::Message;
use crate::config;
use crate::ui::theme;

pub struct SettingsView<'a> {
    pub username: &'a str,
    pub album_count: usize,
    pub downloaded_count: usize,
    pub scroll: u16,
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

        let block = Block::default()
            .title(" Info ")
            .title_style(theme::title())
            .borders(Borders::ALL)
            .border_style(theme::dim());

        let inner = block.inner(area);
        block.render(area, buf);

        let mut all_lines: Vec<Line> = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Version:      ", theme::dim()),
                Span::styled(env!("CARGO_PKG_VERSION"), theme::normal()),
            ]),
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
                Span::styled(config_dir, theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("  Cache:        ", theme::dim()),
                Span::styled(cache_dir, theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("  Library:      ", theme::dim()),
                Span::styled(library_dir, theme::normal()),
            ]),
            Line::from(""),
            Line::from(Span::styled("  Keybindings", theme::selected())),
            Line::from(""),
        ];

        // Render keybindings as text lines so they participate in Paragraph scroll.
        let bindings = Message::all_keybindings();
        let col_width = 28usize;
        let num_cols = ((inner.width as usize) / col_width).max(1);
        let num_rows = bindings.len().div_ceil(num_cols);

        for row in 0..num_rows {
            let mut spans = vec![Span::raw("  ")];
            for col in 0..num_cols {
                let idx = col * num_rows + row;
                if let Some((key, desc)) = bindings.get(idx) {
                    spans.push(Span::styled(format!("{:<8}", key), theme::normal()));
                    spans.push(Span::styled(
                        format!("{:<width$}", desc, width = col_width - 8),
                        theme::dim(),
                    ));
                }
            }
            all_lines.push(Line::from(spans));
        }

        Paragraph::new(all_lines)
            .scroll((self.scroll, 0))
            .render(inner, buf);
    }
}
