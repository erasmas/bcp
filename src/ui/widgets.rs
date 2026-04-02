use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::Widget,
};

use super::theme;

/// A simple progress bar widget
pub struct ProgressBar {
    pub ratio: f64, // 0.0 to 1.0
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 {
            return;
        }
        let filled = (self.ratio * area.width as f64) as u16;
        for x in 0..area.width {
            let (symbol, style) = if x < filled {
                ("\u{2588}", theme::progress_filled()) // Full block
            } else {
                ("\u{2591}", theme::progress_empty()) // Light shade
            };
            buf.set_string(area.x + x, area.y, symbol, style);
        }
    }
}

pub fn format_duration(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}
