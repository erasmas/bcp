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
    pub filtered_indices: &'a [usize],
}

impl<'a> StatefulWidget for CollectionView<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .filter_map(|&i| self.albums.get(i))
            .map(|album| {
                let line = Line::from(vec![
                    Span::styled(&album.artist_name, theme::normal()),
                    Span::styled(" - ", theme::dim()),
                    Span::styled(&album.album_title, theme::normal()),
                ]);
                ListItem::new(line)
            })
            .collect();

        let title = format!(" Collection ({}) ", self.filtered_indices.len());

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
