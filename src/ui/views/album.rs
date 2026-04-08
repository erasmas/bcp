use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph,
        StatefulWidget, Widget,
    },
};

use crate::bandcamp::models::Album;
use crate::library::LibraryIndex;
use crate::ui::theme;
use crate::ui::widgets::{draw_vscrollbar, format_duration, render_list_independent};

pub struct TrackColumn<'a> {
    pub album: Option<&'a Album>,
    pub playing_album_id: Option<u64>,
    pub playing_track_num: Option<u32>,
    pub filtered_indices: &'a [usize],
    pub library: &'a LibraryIndex,
    pub focused: bool,
    pub loading: bool,
}

impl<'a> StatefulWidget for TrackColumn<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let border_style = if self.focused {
            theme::selected()
        } else {
            theme::dim()
        };

        let Some(album) = self.album else {
            let block = Block::default()
                .title(" Tracks ")
                .title_style(theme::title())
                .borders(Borders::ALL)
                .border_style(border_style);
            let paragraph = Paragraph::new("").block(block);
            paragraph.render(area, buf);
            return;
        };

        if album.tracks.is_empty() {
            let block = Block::default()
                .title(" Tracks ")
                .title_style(theme::title())
                .borders(Borders::ALL)
                .border_style(border_style);
            let msg = if self.loading {
                "  Loading..."
            } else {
                "  Press Enter to load tracks"
            };
            let paragraph = Paragraph::new(msg).style(theme::dim()).block(block);
            paragraph.render(area, buf);
            return;
        }

        let is_playing_album = self.playing_album_id == Some(album.item_id);

        let items: Vec<ListItem> =
            self.filtered_indices
                .iter()
                .filter_map(|&i| album.tracks.get(i))
                .map(|track| {
                    let is_playing = is_playing_album
                        && self.playing_track_num.is_some_and(|n| n == track.track_num);

                    let style = if is_playing {
                        theme::playing()
                    } else {
                        theme::normal()
                    };

                    let dl_indicator = if self.album.is_some_and(|a| {
                        self.library.is_track_downloaded(a.item_id, track.track_num)
                    }) {
                        Span::styled("\u{2913} ", theme::playing())
                    } else {
                        Span::raw("  ")
                    };

                    let line = Line::from(vec![
                        dl_indicator,
                        Span::styled(format!("{:2}. ", track.track_num), theme::dim()),
                        Span::styled(&track.title, style),
                        Span::styled(
                            format!("  {}", format_duration(track.duration)),
                            theme::dim(),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();

        let title = format!(" {} - {} ", album.artist_name, album.album_title);

        let list = List::new(items)
            .block(
                Block::default()
                    .title(title)
                    .title_style(theme::title())
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .padding(Padding::right(1)),
            )
            .highlight_style(
                ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            )
            .highlight_symbol("> ")
            .highlight_spacing(HighlightSpacing::Always);

        render_list_independent(list, area, buf, state);

        if area.height > 2 && area.width >= 2 {
            draw_vscrollbar(
                buf,
                Rect::new(area.x + area.width - 2, area.y + 1, 1, area.height - 2),
                self.filtered_indices.len(),
                (area.height - 2) as usize,
                state.offset(),
                self.focused,
            );
        }
    }
}
