use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::player::queue::QueueItem;
use crate::ui::theme;
use crate::ui::widgets::{format_duration, ProgressBar};

/// Minimum percentage of the now-playing bar width reserved for album art
const ART_PERCENT: u16 = 30;

pub struct NowPlayingBar<'a> {
    pub current: Option<&'a QueueItem>,
    pub is_paused: bool,
    pub elapsed: f64,
    pub volume: f32,
    pub has_art: bool,
}

impl<'a> NowPlayingBar<'a> {
    /// Returns the Rect where album art should be rendered (inside the block)
    pub fn art_area(outer: Rect) -> Rect {
        let block = Block::default().borders(Borders::ALL);
        let inner = block.inner(outer);
        let art_width = (inner.width * ART_PERCENT / 100).max(1);
        Rect {
            x: inner.x,
            y: inner.y,
            width: art_width.min(inner.width),
            height: inner.height,
        }
    }
}

impl<'a> Widget for NowPlayingBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Now Playing ")
            .title_style(theme::title())
            .borders(Borders::ALL)
            .border_style(theme::dim());

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        match self.current {
            Some(item) => {
                let art_width = inner.width * ART_PERCENT / 100;
                let art_offset = if self.has_art { art_width + 1 } else { 0 };
                let info_area = Rect {
                    x: inner.x + art_offset,
                    y: inner.y,
                    width: inner.width.saturating_sub(art_offset),
                    height: inner.height,
                };
                render_playing(info_area, buf, item, self.is_paused, self.elapsed, self.volume);
            }
            None => {
                let msg = "No track playing \u{2014} select an album and press Enter";
                buf.set_string(inner.x + 1, inner.y, msg, theme::dim());
            }
        }
    }
}

fn render_playing(
    area: Rect,
    buf: &mut Buffer,
    item: &QueueItem,
    is_paused: bool,
    elapsed: f64,
    volume: f32,
) {
    if area.width < 5 || area.height < 1 {
        return;
    }

    let icon = if is_paused { "\u{23F8}" } else { "\u{25B6}" };
    let duration = item.track.duration;
    let time_str = format!(
        "[{}/{}]",
        format_duration(elapsed),
        format_duration(duration)
    );

    // Line 1: icon + artist - title + time
    let title_line = Line::from(vec![
        Span::styled(format!(" {} ", icon), theme::playing()),
        Span::styled(&item.artist_name, theme::normal()),
        Span::styled(" - ", theme::dim()),
        Span::styled(&item.track.title, theme::normal()),
        Span::styled(format!("  {}", time_str), theme::dim()),
    ]);
    buf.set_line(area.x, area.y, &title_line, area.width);

    // Line 2: album name
    if area.height > 1 {
        let album_line = Line::from(vec![
            Span::styled("   ", theme::normal()),
            Span::styled(&item.album_title, theme::dim()),
        ]);
        buf.set_line(area.x, area.y + 1, &album_line, area.width);
    }

    // Line 3: progress bar + volume
    if area.height > 2 {
        let vol_str = format!("Vol: {}%", (volume * 100.0) as u32);
        let vol_width = vol_str.len() as u16 + 2;
        let bar_width = area.width.saturating_sub(vol_width + 3);

        let chunks = Layout::horizontal([
            Constraint::Length(3),
            Constraint::Length(bar_width),
            Constraint::Length(2),
            Constraint::Min(vol_width),
        ])
        .split(Rect {
            x: area.x,
            y: area.y + 2,
            width: area.width,
            height: 1,
        });

        let ratio = if duration > 0.0 {
            (elapsed / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };
        ProgressBar { ratio }.render(chunks[1], buf);
        buf.set_string(chunks[3].x, chunks[3].y, &vol_str, theme::dim());
    }
}
