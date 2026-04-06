use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

use crate::bandcamp::models::Album;
use crate::library::{AlbumDownloadStatus, LibraryIndex};
use crate::ui::theme;

pub struct AlbumColumn<'a> {
    pub albums: &'a [Album],
    /// Indices into the artist's album list (which itself indexes into albums)
    pub album_indices: &'a [usize],
    pub filtered_indices: &'a [usize],
    pub library: &'a LibraryIndex,
    pub focused: bool,
}

impl<'a> StatefulWidget for AlbumColumn<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .filter_map(|&i| self.album_indices.get(i))
            .filter_map(|&album_idx| self.albums.get(album_idx))
            .map(|album| {
                let indicator = match self.library.album_status(album.item_id) {
                    Some(AlbumDownloadStatus::Complete) => {
                        Span::styled("\u{2913} ", theme::playing())
                    }
                    Some(AlbumDownloadStatus::Downloading | AlbumDownloadStatus::Partial) => {
                        Span::styled("\u{2913} ", theme::playing())
                    }
                    None => Span::raw("  "),
                };
                let line = Line::from(vec![
                    indicator,
                    Span::styled(album.album_title.as_str(), theme::normal()),
                ]);
                ListItem::new(line)
            })
            .collect();

        let title = format!(" Albums ({}) ", self.filtered_indices.len());

        let border_style = if self.focused {
            theme::selected()
        } else {
            theme::dim()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .title(title)
                    .title_style(theme::title())
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .highlight_style(ratatui::style::Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        StatefulWidget::render(list, area, buf, state);
    }
}
