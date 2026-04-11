use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::player::queue::QueueItem;
use crate::ui::theme;
use crate::ui::widgets::{draw_vscrollbar, format_duration};

/// Minimum percentage of the now-playing bar width reserved for album art
const ART_PERCENT: u16 = 30;

pub struct NowPlayingBar<'a> {
    pub current: Option<&'a QueueItem>,
    pub is_paused: bool,
    pub elapsed: f64,
    pub has_art: bool,
    pub meta_scroll: usize,
    pub stream_bitrate: Option<&'a str>,
    pub queue_len: usize,
    pub queue_total: f64,
}

impl<'a> NowPlayingBar<'a> {
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
                    self.meta_scroll,
                    self.stream_bitrate,
                    self.queue_len,
                    self.queue_total,
                );
            }
            None => {
                render_idle(inner, buf);
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
    meta_scroll: usize,
    stream_bitrate: Option<&str>,
    queue_len: usize,
    queue_total: f64,
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

    // Line 1: icon + artist - track title + time | queue info | bitrate (right-aligned)
    let format_label = stream_bitrate
        .map(|b| format!(" {} ", b))
        .unwrap_or_default();
    let queue_label = if queue_len > 0 {
        format!(" {} \u{00b7} {} ", queue_len, format_duration(queue_total))
    } else {
        String::new()
    };
    let label_width = format_label.len() as u16;
    let queue_width = queue_label.len() as u16;
    let title_width = area.width.saturating_sub(label_width + queue_width);
    let title_line = Line::from(vec![
        Span::styled(format!(" {} ", icon), theme::playing()),
        Span::styled(&item.artist_name, theme::normal()),
        Span::styled(" - ", theme::dim()),
        Span::styled(&item.track.title, theme::normal()),
        Span::styled(format!("  {}", time_str), theme::dim()),
    ]);
    buf.set_line(area.x, area.y, &title_line, title_width);
    if queue_width > 0 {
        buf.set_string(area.x + title_width, area.y, &queue_label, theme::dim());
    }
    if label_width > 0 {
        buf.set_string(
            area.x + title_width + queue_width,
            area.y,
            &format_label,
            theme::dim(),
        );
    }

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

    // Line 3+: album metadata (about, credits, release date) - auto-scrolling
    if area.height > 3 {
        // Reserve 1 column on the right for a scrollbar.
        let max_width = area.width.saturating_sub(5) as usize;
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

        let max_scroll = all_lines.len().saturating_sub(visible_rows);
        let scroll_offset = meta_scroll.min(max_scroll);

        // Render visible window
        let visible = &all_lines[scroll_offset..];
        for (i, line) in visible.iter().enumerate() {
            if i >= visible_rows {
                break;
            }
            let y = area.y + 3 + i as u16;
            buf.set_string(area.x + 2, y, line, theme::normal());
        }

        if area.width >= 2 {
            draw_vscrollbar(
                buf,
                Rect::new(area.x + area.width - 2, area.y + 3, 1, visible_rows as u16),
                all_lines.len(),
                visible_rows,
                scroll_offset,
                true,
            );
        }
    }
}

fn format_release_date(date: &str) -> String {
    // Bandcamp dates are like "16 Jan 2026 00:00:00 GMT" - extract the readable part
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

use crate::ui::logo::{LOGO, logo_gradient};

fn render_idle(area: Rect, buf: &mut Buffer) {
    let logo_height = LOGO.len() as u16;
    let logo_width = LOGO.first().map(|l| l.len()).unwrap_or(0) as u16;

    let y_offset = area.height.saturating_sub(logo_height) / 2;
    let x_offset = area.width.saturating_sub(logo_width) / 2;

    let gradient = logo_gradient(LOGO.len());
    for (i, line) in LOGO.iter().enumerate() {
        let y = area.y + y_offset + i as u16;
        if y >= area.y + area.height {
            break;
        }
        let style = Style::default()
            .fg(gradient[i])
            .add_modifier(ratatui::style::Modifier::BOLD);
        buf.set_string(area.x + x_offset, y, line, style);
    }
}
