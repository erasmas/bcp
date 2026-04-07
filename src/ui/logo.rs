use ratatui::style::Color;

pub const LOGO: &[&str] = &[
    "████                                         ",
    "████                                         ",
    "█████████████   █████████████   █████████████",
    "█████████████   █████████████   █████████████",
    "████     ████   ████            ████     ████",
    "████     ████   ████            ████     ████",
    "█████████████   █████████████   █████████████",
    "█████████████   █████████████   █████████████",
    "                                ████         ",
    "                                ████         ",
];

pub fn logo_gradient(num_lines: usize) -> Vec<Color> {
    let start = (136u8, 192u8, 208u8); // Nord8 cyan
    let end = (180u8, 142u8, 173u8); // Nord15 purple
    (0..num_lines)
        .map(|i| {
            let t = if num_lines <= 1 {
                0.0
            } else {
                i as f32 / (num_lines - 1) as f32
            };
            Color::Rgb(
                (start.0 as f32 + (end.0 as f32 - start.0 as f32) * t) as u8,
                (start.1 as f32 + (end.1 as f32 - start.1 as f32) * t) as u8,
                (start.2 as f32 + (end.2 as f32 - start.2 as f32) * t) as u8,
            )
        })
        .collect()
}
