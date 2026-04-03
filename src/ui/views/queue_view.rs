use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

use crate::player::queue::PlayQueue;
use crate::ui::theme;
use crate::ui::widgets::format_duration;

pub struct QueueView<'a> {
    pub queue: &'a PlayQueue,
}

impl<'a> StatefulWidget for QueueView<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let current_idx = self.queue.current;

        let items: Vec<ListItem> = self
            .queue
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_current = current_idx == Some(i);
                let prefix = if is_current { "\u{25B6} " } else { "  " };
                let style = if is_current {
                    theme::playing()
                } else {
                    theme::normal()
                };

                let line = Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(&item.artist_name, style),
                    Span::styled(" - ", theme::dim()),
                    Span::styled(&item.track.title, style),
                    Span::styled(
                        format!("  [{}]", &item.album_title),
                        theme::dim(),
                    ),
                    Span::styled(
                        format!("  {}", format_duration(item.track.duration)),
                        theme::dim(),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let shuffle_indicator = if self.queue.shuffle { " [S]" } else { "" };
        let repeat_indicator = if self.queue.repeat { " [R]" } else { "" };
        let title = format!(
            " Queue ({}) {}{} ",
            self.queue.items.len(),
            shuffle_indicator,
            repeat_indicator
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
