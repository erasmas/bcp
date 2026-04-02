use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::player::queue::QueueItem;
use crate::ui::theme;
use crate::ui::widgets::{format_duration, Waveform};

/// Minimum percentage of the now-playing bar width reserved for album art
const ART_PERCENT: u16 = 30;

pub struct NowPlayingBar<'a> {
    pub current: Option<&'a QueueItem>,
    pub is_paused: bool,
    pub elapsed: f64,
    pub has_art: bool,
    pub waveform: &'a [u64],
}

impl<'a> NowPlayingBar<'a> {
    /// Compute the ideal height for the now-playing section based on album art size.
    /// Art is ART_PERCENT% of terminal width; square cover needs height = width/2.
    /// Add 2 for borders.
    pub fn ideal_height(terminal_width: u16) -> u16 {
        let inner_width = terminal_width.saturating_sub(2); // borders
        let art_cols = inner_width * ART_PERCENT / 100;
        // Square cover in terminal: height ≈ width * 0.38 (empirical, accounts
        // for actual cell aspect ratio)
        let art_rows = (art_cols as f64 * 0.38) as u16;
        art_rows + 2 // add borders
    }

    /// Returns the Rect where album art should be rendered, vertically centered.
    pub fn art_area(outer: Rect) -> Rect {
        let block = Block::default().borders(Borders::ALL);
        let inner = block.inner(outer);
        let art_width = (inner.width * ART_PERCENT / 100).max(1);
        let img_height = inner.height;
        Rect {
            x: inner.x,
            y: inner.y,
            width: art_width.min(inner.width),
            height: img_height,
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
                render_playing(
                    info_area,
                    buf,
                    item,
                    self.is_paused,
                    self.elapsed,
                    self.waveform,
                );
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
    waveform: &[u64],
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
        Span::styled(" \u{2014} ", theme::dim()),
        Span::styled(&item.track.title, theme::normal()),
        Span::styled(format!("  {}", time_str), theme::dim()),
    ]);
    buf.set_line(area.x, area.y, &title_line, area.width);

    // Line 2: thin progress line (single row, heatmap color)
    if area.height > 1 {
        let ratio = if duration > 0.0 {
            (elapsed / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let bar_width = area.width.saturating_sub(2);
        let filled = (ratio * bar_width as f64) as u16;
        for x in 0..bar_width {
            let style = if x < filled {
                Style::default().fg(Color::Rgb(120, 200, 170))
            } else {
                Style::default().fg(Color::Rgb(46, 52, 64))
            };
            buf.set_string(area.x + 1 + x, area.y + 1, "\u{2500}", style);
        }
    }

    // Remaining lines: scrolling 5-second waveform heatmap
    if area.height > 2 && !waveform.is_empty() {
        let wave_area = Rect {
            x: area.x + 1,
            y: area.y + 2,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        if wave_area.width > 0 && wave_area.height > 0 {
            render_waveform_window(wave_area, buf, waveform, elapsed, duration);
        }
    }
}

/// Render a scrolling 5-second waveform window centered on the playhead.
/// Waveform data is at 100 points per second (10ms per point).
fn render_waveform_window(
    area: Rect,
    buf: &mut Buffer,
    waveform: &[u64],
    elapsed: f64,
    _duration: f64,
) {
    const POINTS_PER_SEC: f64 = 100.0;
    const WINDOW_SECS: f64 = 5.0;

    let display_width = area.width as usize;
    let total_points = waveform.len();
    if total_points == 0 || display_width == 0 {
        return;
    }

    let current_point = (elapsed * POINTS_PER_SEC) as usize;

    // Window: 1s behind, 4s ahead
    let window_points = (WINDOW_SECS * POINTS_PER_SEC) as usize;
    let behind_points = (1.0 * POINTS_PER_SEC) as usize;
    let window_start = current_point.saturating_sub(behind_points);
    let window_end = (window_start + window_points).min(total_points);

    let window = &waveform[window_start..window_end];
    if window.is_empty() {
        return;
    }

    let resampled = resample(window, display_width);

    Waveform {
        data: &resampled,
    }
    .render(area, buf);
}

fn resample(src: &[u64], target_len: usize) -> Vec<u64> {
    if target_len == 0 || src.is_empty() {
        return vec![0; target_len];
    }

    let src_len = src.len();
    (0..target_len)
        .map(|i| {
            let start = i * src_len / target_len;
            let end = ((i + 1) * src_len / target_len).max(start + 1).min(src_len);
            let sum: u64 = src[start..end].iter().sum();
            let count = (end - start) as u64;
            if count > 0 { sum / count } else { 0 }
        })
        .collect()
}
