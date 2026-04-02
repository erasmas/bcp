use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

use crate::bandcamp::models::Album;
use crate::ui::theme;
use crate::ui::widgets::format_duration;

pub struct AlbumView<'a> {
    pub album: &'a Album,
    pub playing_track_num: Option<u32>,
}

impl<'a> StatefulWidget for AlbumView<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let items: Vec<ListItem> = self
            .album
            .tracks
            .iter()
            .map(|track| {
                let is_playing = self
                    .playing_track_num
                    .is_some_and(|n| n == track.track_num);

                let prefix = if is_playing { "\u{25B6} " } else { "  " };
                let style = if is_playing {
                    theme::playing()
                } else {
                    theme::normal()
                };

                let line = Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(
                        format!("{:2}. ", track.track_num),
                        theme::dim(),
                    ),
                    Span::styled(&track.title, style),
                    Span::styled(
                        format!("  {}", format_duration(track.duration)),
                        theme::dim(),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let title = format!(
            " {} - {} ",
            self.album.artist_name, self.album.album_title
        );

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
