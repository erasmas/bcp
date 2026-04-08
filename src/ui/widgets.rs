use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::{List, ListState, StatefulWidget};

use crate::ui::theme;

pub fn format_duration(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

/// Render a `List` while keeping the scroll offset independent of the
/// selection. Ratatui's `List` normally adjusts `state.offset` on render to
/// keep `state.selected` in view; we want lazygit-style behavior where the
/// scroll wheel scrolls the viewport without dragging the cursor along.
///
/// Trick: clear the selection during render (no selection -> no auto-scroll),
/// then re-paint the highlight ourselves at the visible row, if any.
///
/// The list MUST be built with `HighlightSpacing::Always` so the row layout
/// stays consistent regardless of whether anything is selected.
pub fn render_list_independent(
    list: List<'_>,
    area: Rect,
    buf: &mut Buffer,
    state: &mut ListState,
) {
    // ListState::select(None) zeroes the offset (see ratatui-widgets list/state.rs:159),
    // which would clobber our scroll position. Use selected_mut() to bypass that.
    let real_sel = state.selected();
    *state.selected_mut() = None;
    StatefulWidget::render(list, area, buf, state);
    *state.selected_mut() = real_sel;

    if area.height < 3 || area.width < 4 {
        return;
    }
    let Some(sel) = real_sel else {
        return;
    };
    let offset = state.offset();
    let visible = (area.height - 2) as usize;
    if sel < offset || sel >= offset + visible {
        return;
    }

    let row = area.y + 1 + (sel - offset) as u16;
    // Highlight symbol "> " takes 2 cells.
    buf.set_string(area.x + 1, row, "> ", theme::normal());
    // Bold the row, leaving the right border + scrollbar cell free.
    let row_end = area.x + area.width - 2;
    for x in (area.x + 1)..row_end {
        buf[Position::new(x, row)].modifier.insert(Modifier::BOLD);
    }
}

/// Draw a thin vertical scrollbar in `track` (a 1-cell-wide rect).
///
/// `total` is the total number of items, `visible` is how many fit at once,
/// `offset` is the index of the first visible item.
///
/// Renders nothing if everything fits (`total <= visible`).
pub fn draw_vscrollbar(
    buf: &mut Buffer,
    track: Rect,
    total: usize,
    visible: usize,
    offset: usize,
    focused: bool,
) {
    if track.height == 0 || visible == 0 || total <= visible {
        return;
    }

    let track_h = track.height as usize;
    let thumb_h = ((visible * track_h) / total).max(1).min(track_h);
    let max_scroll = total.saturating_sub(visible);
    let max_thumb_off = track_h - thumb_h;
    let thumb_y = if max_scroll == 0 {
        0
    } else {
        (offset * max_thumb_off + max_scroll / 2) / max_scroll
    };

    let thumb_style = if focused {
        theme::selected()
    } else {
        theme::dim()
    };

    for i in 0..track_h {
        let yy = track.y + i as u16;
        let in_thumb = i >= thumb_y && i < thumb_y + thumb_h;
        let (ch, style) = if in_thumb {
            ("\u{2590}", thumb_style)
        } else {
            (" ", theme::dim())
        };
        buf.set_string(track.x, yy, ch, style);
    }
}
