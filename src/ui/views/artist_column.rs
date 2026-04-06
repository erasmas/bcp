use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

use crate::ui::theme;

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
                    .border_style(border_style),
            )
            .highlight_style(theme::selected())
            .highlight_symbol("> ");

        StatefulWidget::render(list, area, buf, state);
    }
}
