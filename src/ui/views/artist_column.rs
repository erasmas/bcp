use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Padding, StatefulWidget,
    },
};

use crate::ui::theme;
use crate::ui::widgets::{draw_vscrollbar, render_list_independent};

pub struct ArtistColumn<'a> {
    pub artists: &'a [String],
    pub filtered_indices: &'a [usize],
    pub focused: bool,
}

impl<'a> StatefulWidget for ArtistColumn<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .filter_map(|&i| self.artists.get(i))
            .map(|name| ListItem::new(name.as_str()).style(theme::normal()))
            .collect();

        let title = format!(" Artists ({}) ", self.filtered_indices.len());

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
