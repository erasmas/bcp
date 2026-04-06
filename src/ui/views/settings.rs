use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, Widget},
};

use crate::app::Message;
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

        let block = Block::default()
            .title(" Info ")
            .title_style(theme::title())
            .borders(Borders::ALL)
            .border_style(theme::dim());

        let inner = block.inner(area);
        block.render(area, buf);

        // Info section
        let info_lines = vec![
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
        ];

        let info_height = info_lines.len() as u16;
        let info_area = Rect::new(inner.x, inner.y, inner.width, info_height.min(inner.height));
        Paragraph::new(info_lines).render(info_area, buf);

        // Keybindings table
        if inner.height > info_height {
            let table_area = Rect::new(
                inner.x + 2,
                inner.y + info_height,
                inner.width.saturating_sub(4),
                inner.height - info_height,
            );

            let bindings = Message::all_keybindings();
            let col_width = 28u16;
            let num_cols = (table_area.width / col_width).max(1) as usize;
            let num_rows = bindings.len().div_ceil(num_cols);

            let rows: Vec<Row> = (0..num_rows)
                .map(|row| {
                    let mut cells = Vec::new();
                    for col in 0..num_cols {
                        let idx = col * num_rows + row;
                        if let Some((key, desc)) = bindings.get(idx) {
                            cells.push(Span::styled(*key, theme::normal()));
                            cells.push(Span::styled(*desc, theme::dim()));
                        }
                    }
                    Row::new(cells)
                })
                .collect();

            let widths: Vec<Constraint> = (0..num_cols)
                .flat_map(|_| [Constraint::Length(8), Constraint::Length(col_width - 8)])
                .collect();

            let table = Table::new(rows, widths);
            Widget::render(table, table_area, buf);
        }
    }
}
