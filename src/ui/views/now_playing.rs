use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::player::queue::QueueItem;
use crate::ui::theme;
use crate::ui::widgets::format_duration;

/// Minimum percentage of the now-playing bar width reserved for album art
const ART_PERCENT: u16 = 30;

pub struct NowPlayingBar<'a> {
    pub current: Option<&'a QueueItem>,
    pub is_paused: bool,
    pub elapsed: f64,
    pub has_art: bool,
    pub meta_scroll: Option<usize>,  // None = auto, Some(n) = manual offset
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
                render_playing(info_area, buf, item, self.is_paused, self.elapsed, self.meta_scroll);
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
    meta_scroll: Option<usize>,
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

    // Line 1: icon + artist — track title + time
    let title_line = Line::from(vec![
        Span::styled(format!(" {} ", icon), theme::playing()),
        Span::styled(&item.artist_name, theme::normal()),
        Span::styled(" \u{2014} ", theme::dim()),
        Span::styled(&item.track.title, theme::normal()),
        Span::styled(format!("  {}", time_str), theme::dim()),
    ]);
    buf.set_line(area.x, area.y, &title_line, area.width);

    // Line 2: thin progress line
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

    // Line 3+: album metadata (about, credits, release date) — auto-scrolling
    if area.height > 3 {
        let max_width = area.width.saturating_sub(4) as usize;
        let visible_rows = (area.height - 3) as usize;

        // Build all text lines
        let mut all_lines: Vec<String> = Vec::new();

        // Album title
        all_lines.push(item.album_title.clone());

        // Release date
        if let Some(ref date) = item.release_date {
            all_lines.push(format!("released {}", format_release_date(date)));
        }

        all_lines.push(String::new()); // separator

        // About
        if let Some(ref about) = item.about {
            for line in word_wrap(about, max_width) {
                all_lines.push(line);
            }
        }

        // Credits
        if let Some(ref credits) = item.credits {
            all_lines.push(String::new());
            for line in word_wrap(credits, max_width) {
                all_lines.push(line);
            }
        }

        // Scroll offset: manual if set, otherwise auto-scroll 1 line per 2s
        let max_scroll = all_lines.len().saturating_sub(visible_rows);
        let scroll_offset = match meta_scroll {
            Some(manual) => manual.min(max_scroll),
            None => {
                if all_lines.len() > visible_rows {
                    ((elapsed / 2.0) as usize).min(max_scroll)
                } else {
                    0
                }
            }
        };

        // Render visible window
        let visible = &all_lines[scroll_offset..];
        for (i, line) in visible.iter().enumerate() {
            if i >= visible_rows {
                break;
            }
            let y = area.y + 3 + i as u16;
            buf.set_string(area.x + 2, y, line, theme::normal());
        }
    }
}

fn format_release_date(date: &str) -> String {
    // Bandcamp dates are like "16 Jan 2026 00:00:00 GMT" — extract the readable part
    let parts: Vec<&str> = date.split_whitespace().collect();
    if parts.len() >= 3 {
        format!("{} {} {}", parts[0], parts[1], parts[2])
    } else {
        date.to_string()
    }
}

fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    for paragraph in text.lines() {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in paragraph.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= max_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    lines
}
