use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

use crate::bandcamp::models::Album;
use crate::ui::theme;

pub struct CollectionView<'a> {
    pub albums: &'a [Album],
    pub filter: &'a str,
}

impl<'a> CollectionView<'a> {
    pub fn filtered_albums(&self) -> Vec<(usize, &'a Album)> {
        if self.filter.is_empty() {
            self.albums.iter().enumerate().collect()
        } else {
            let q = self.filter.to_lowercase();
            self.albums
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    a.album_title.to_lowercase().contains(&q)
                        || a.artist_name.to_lowercase().contains(&q)
                })
                .collect()
        }
    }
}

impl<'a> StatefulWidget for CollectionView<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let filtered = self.filtered_albums();

        let items: Vec<ListItem> = filtered
            .iter()
            .map(|(_, album)| {
                let line = Line::from(vec![
                    Span::styled(&album.artist_name, theme::normal()),
                    Span::styled(" - ", theme::dim()),
                    Span::styled(&album.album_title, theme::normal()),
                ]);
                ListItem::new(line)
            })
            .collect();

        let title = if self.filter.is_empty() {
            format!(" Collection ({}) ", filtered.len())
        } else {
            format!(" Collection ({}) [/{}] ", filtered.len(), self.filter)
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .title(title)
                    .title_style(theme::title())
                    .borders(Borders::ALL)
                    .border_style(theme::dim()),
            )
            .highlight_style(theme::selected())
            .highlight_symbol("> ");

        StatefulWidget::render(list, area, buf, state);
    }
}
