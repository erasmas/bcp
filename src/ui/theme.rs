use ratatui::style::{Color, Modifier, Style};

// Nord-inspired color scheme
pub const BG_ALT: Color = Color::Rgb(59, 66, 82);   // Nord1
pub const FG: Color = Color::Rgb(216, 222, 233);     // Nord4
pub const FG_DIM: Color = Color::Rgb(76, 86, 106);   // Nord3
pub const ACCENT: Color = Color::Rgb(136, 192, 208); // Nord8
pub const HIGHLIGHT: Color = Color::Rgb(163, 190, 140); // Nord14 green

pub fn normal() -> Style {
    Style::default().fg(FG)
}

pub fn dim() -> Style {
    Style::default().fg(FG_DIM)
}

pub fn selected() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn playing() -> Style {
    Style::default().fg(HIGHLIGHT).add_modifier(Modifier::BOLD)
}

pub fn title() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn status_bar() -> Style {
    Style::default().fg(FG).bg(BG_ALT)
}

pub fn progress_filled() -> Style {
    Style::default().fg(HIGHLIGHT)
}

pub fn progress_empty() -> Style {
    Style::default().fg(FG_DIM)
}
