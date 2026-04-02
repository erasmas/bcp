use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::player::queue::QueueItem;
use crate::ui::theme;
use crate::ui::widgets::{format_duration, ProgressBar};

pub struct NowPlayingBar<'a> {
    pub current: Option<&'a QueueItem>,
    pub is_paused: bool,
    pub elapsed: f64,
    pub volume: f32,
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
                render_playing(inner, buf, item, self.is_paused, self.elapsed, self.volume);
            }
            None => {
                let msg = "No track playing — select an album and press Enter";
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
    // Layout: leave left space for potential art, rest for info
    let art_width = 0_u16; // Art rendering via viuer is handled separately
    let info_area = Rect {
        x: area.x + art_width + 1,
        y: area.y,
        width: area.width.saturating_sub(art_width + 1),
        height: area.height,
    };

    if info_area.width < 5 || info_area.height < 1 {
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
    buf.set_line(info_area.x, info_area.y, &title_line, info_area.width);

    // Line 2: album name
    if info_area.height > 1 {
        let album_line = Line::from(vec![
            Span::styled("   ", theme::normal()),
            Span::styled(&item.album_title, theme::dim()),
        ]);
        buf.set_line(info_area.x, info_area.y + 1, &album_line, info_area.width);
    }

    // Line 3: progress bar + volume
    if info_area.height > 2 {
        let vol_str = format!("Vol: {}%", (volume * 100.0) as u32);
        let vol_width = vol_str.len() as u16 + 2;
        let bar_width = info_area.width.saturating_sub(vol_width + 3);

        let chunks = Layout::horizontal([
            Constraint::Length(3), // padding
            Constraint::Length(bar_width),
            Constraint::Length(2), // gap
            Constraint::Min(vol_width),
        ])
        .split(Rect {
            x: info_area.x,
            y: info_area.y + 2,
            width: info_area.width,
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
