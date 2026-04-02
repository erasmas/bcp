use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

pub fn format_duration(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

/// A heatmap waveform widget. Each column is a time slice, rows light up
/// from center outward based on amplitude. Color goes from cool to hot.
pub struct Waveform<'a> {
    pub data: &'a [u64],
}

impl<'a> Widget for Waveform<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.data.is_empty() {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;
        let center = height as f64 / 2.0;

        for col in 0..width.min(self.data.len()) {
            let amp = self.data[col].min(100) as f64 / 100.0;
            let reach = amp * center;

            for row in 0..height {
                let dist_from_center = (row as f64 - center + 0.5).abs();

                if dist_from_center > reach {
                    continue;
                }

                let intensity = if reach > 0.0 {
                    1.0 - (dist_from_center / reach)
                } else {
                    0.0
                };

                let color = heat_color(intensity, amp);

                buf.set_string(
                    area.x + col as u16,
                    area.y + row as u16,
                    " ",
                    Style::default().bg(color),
                );
            }
        }
    }
}

/// Gradient: dark teal → green → cyan → bright based on intensity and amplitude
fn heat_color(intensity: f64, amp: f64) -> Color {
    let t = (intensity * 0.6 + amp * 0.4).clamp(0.0, 1.0);

    let (r, g, b) = if t < 0.25 {
        let f = t / 0.25;
        lerp_rgb((25, 40, 45), (46, 90, 80), f)
    } else if t < 0.5 {
        let f = (t - 0.25) / 0.25;
        lerp_rgb((46, 90, 80), (76, 145, 120), f)
    } else if t < 0.75 {
        let f = (t - 0.5) / 0.25;
        lerp_rgb((76, 145, 120), (120, 200, 170), f)
    } else {
        let f = (t - 0.75) / 0.25;
        lerp_rgb((120, 200, 170), (180, 235, 210), f)
    };

    Color::Rgb(r, g, b)
}

fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let r = a.0 as f64 + (b.0 as f64 - a.0 as f64) * t;
    let g = a.1 as f64 + (b.1 as f64 - a.1 as f64) * t;
    let b_val = a.2 as f64 + (b.2 as f64 - a.2 as f64) * t;
    (r as u8, g as u8, b_val as u8)
}
