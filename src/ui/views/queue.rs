use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, HighlightSpacing, List, ListItem, ListState, Padding,
        Paragraph, StatefulWidget, Widget,
    },
};

use crate::player::queue::QueueItem;
use crate::ui::theme;
use crate::ui::widgets::{draw_vscrollbar, format_duration, render_list_independent};

pub struct QueueColumn<'a> {
    pub items: &'a [QueueItem],
    pub current: Option<usize>,
    pub focused: bool,
}

impl<'a> StatefulWidget for QueueColumn<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let border_style = if self.focused {
            theme::selected()
        } else {
            theme::dim()
        };

        let title = format!(" Queue ({}) ", self.items.len());
        let block = Block::default()
            .title(title)
            .title_style(theme::title())
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(border_style)
            .padding(Padding::right(1));

        if self.items.is_empty() {
            let paragraph = Paragraph::new("  Queue is empty")
                .style(theme::dim())
                .block(block);
            paragraph.render(area, buf);
            return;
        }

        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_current = self.current == Some(i);
                let style = if is_current {
                    theme::playing()
                } else {
                    theme::normal()
                };

                let marker = if is_current {
                    Span::styled("\u{25b6} ", theme::playing())
                } else {
                    Span::raw("  ")
                };

                let line = Line::from(vec![
                    marker,
                    Span::styled(&item.track.title, style),
                    Span::styled(
                        format!("  {}", format_duration(item.track.duration)),
                        theme::dim(),
                    ),
                    Span::styled(
                        format!("  {} \u{2014} {}", item.artist_name, item.album_title),
                        theme::dim(),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(list_items)
            .block(block)
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
                self.items.len(),
                (area.height - 2) as usize,
                state.offset(),
                self.focused,
            );
        }
    }
}
