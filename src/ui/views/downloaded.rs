use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

use crate::bandcamp::models::Album;
use crate::library::{AlbumDownloadStatus, LibraryIndex};
use crate::ui::theme;

pub struct DownloadedView<'a> {
    pub albums: &'a [Album],
    pub library: &'a LibraryIndex,
    pub filter: &'a str,
}

impl<'a> DownloadedView<'a> {
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

impl<'a> StatefulWidget for DownloadedView<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let filtered = self.filtered_albums();

        let items: Vec<ListItem> = filtered
            .iter()
            .map(|(_, album)| {
                let dl = self.library.albums.get(&album.item_id);

                let is_downloaded = dl.is_some_and(|a| a.status == AlbumDownloadStatus::Complete);
                let is_downloading =
                    dl.is_some_and(|a| a.status == AlbumDownloadStatus::Downloading);

                let name_style = if is_downloaded || is_downloading {
                    theme::normal()
                } else {
                    theme::dim()
                };

                let status_text = match dl {
                    Some(a) => match a.status {
                        AlbumDownloadStatus::Complete => {
                            format!("{}/{}", a.tracks.len(), a.tracks.len())
                        }
                        AlbumDownloadStatus::Downloading => {
                            let done = a.tracks.iter().filter(|t| t.downloaded).count();
                            format!("{}/{}  downloading...", done, a.tracks.len())
                        }
                        AlbumDownloadStatus::Partial => {
                            let done = a.tracks.iter().filter(|t| t.downloaded).count();
                            format!("{}/{}", done, a.tracks.len())
                        }
                    },
                    None => String::new(),
                };

                let line = Line::from(vec![
                    Span::styled(&album.artist_name, name_style),
                    Span::styled(" - ", theme::dim()),
                    Span::styled(&album.album_title, name_style),
                    Span::styled(format!("  {}", status_text), theme::dim()),
                ]);
                ListItem::new(line)
            })
            .collect();

        let downloaded_count = self.library.albums.values()
            .filter(|a| a.status == AlbumDownloadStatus::Complete)
            .count();
        let title = format!(" Downloaded ({}) ", downloaded_count);

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
