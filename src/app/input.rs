use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::{App, AppMode, AppScreen, Column, LoginStep, Message};

fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && x >= rect.x
        && x < rect.x + rect.width
        && y >= rect.y
        && y < rect.y + rect.height
}

impl App {
    /// Pure mapping from key event to message. No state mutations.
    pub(crate) fn map_key(&self, key: KeyEvent) -> Option<Message> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Some(Message::Quit);
        }

        match self.screen {
            AppScreen::Login => self.map_login_key(key),
            AppScreen::Loading => None,
            AppScreen::Main => match self.mode {
                AppMode::Settings { .. } => match key.code {
                    KeyCode::Esc | KeyCode::Char('?') => Some(Message::ToggleSettings),
                    KeyCode::Char('q') => Some(Message::Quit),
                    KeyCode::Char('j') | KeyCode::Down => Some(Message::ScrollSettings(1)),
                    KeyCode::Char('k') | KeyCode::Up => Some(Message::ScrollSettings(-1)),
                    _ => None,
                },
                AppMode::Filter => {
                    let is_nav = matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Enter);
                    if !is_nav {
                        return self.map_filter_key(key);
                    }
                    // Enter during filter: confirm filter then fall through to main
                    if key.code == KeyCode::Enter {
                        return Some(Message::ConfirmFilter);
                    }
                    self.map_main_key(key)
                }
                AppMode::Normal => self.map_main_key(key),
            },
        }
    }

    fn map_login_key(&self, key: KeyEvent) -> Option<Message> {
        match self.login_step {
            LoginStep::Prompt => match key.code {
                KeyCode::Enter => Some(Message::OpenLogin),
                KeyCode::Char('q') => Some(Message::Quit),
                _ => None,
            },
            LoginStep::WaitingForBrowser => match key.code {
                KeyCode::Enter => Some(Message::ExtractCookie),
                KeyCode::Char('q') => Some(Message::Quit),
                _ => None,
            },
            LoginStep::Extracting => None,
        }
    }

    fn map_main_key(&self, key: KeyEvent) -> Option<Message> {
        match key.code {
            KeyCode::Char('q') => Some(Message::Quit),
            KeyCode::Esc => {
                if !self.filter_text.is_empty() {
                    Some(Message::CancelFilter)
                } else {
                    Some(Message::FocusLeft)
                }
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::PageDown)
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::PageUp)
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::HalfPageDown)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::HalfPageUp)
            }
            KeyCode::Char('h') | KeyCode::Left => Some(Message::FocusLeft),
            KeyCode::Char('l') | KeyCode::Right => Some(Message::FocusRight),
            KeyCode::Char('?') => Some(Message::ToggleSettings),
            KeyCode::Char('d') => Some(Message::Download),
            KeyCode::Char('D') => Some(Message::DownloadAll),
            KeyCode::Char('j') | KeyCode::Down => Some(Message::MoveDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Message::MoveUp),
            KeyCode::Char('g') => Some(Message::MoveToTop),
            KeyCode::Char('G') => Some(Message::MoveToBottom),
            KeyCode::PageDown => Some(Message::PageDown),
            KeyCode::PageUp => Some(Message::PageUp),
            KeyCode::Enter => Some(Message::Enter),
            KeyCode::Char(' ') => Some(Message::TogglePause),
            KeyCode::Char('n') => Some(Message::NextTrack),
            KeyCode::Char('p') => Some(Message::PrevTrack),
            KeyCode::Char('[') => Some(Message::SeekBackward),
            KeyCode::Char(']') => Some(Message::SeekForward),
            KeyCode::Char('J') => Some(Message::ScrollMetaDown),
            KeyCode::Char('K') => Some(Message::ScrollMetaUp),
            KeyCode::Char('r') => Some(Message::Refresh),
            KeyCode::Char('y') => Some(Message::Yank),
            KeyCode::Char('/') => Some(Message::StartFilter),
            _ => None,
        }
    }

    /// Pure mapping from a mouse event to zero or more messages.
    pub(crate) fn map_mouse(&self, ev: MouseEvent) -> Vec<Message> {
        if self.screen != AppScreen::Main {
            return Vec::new();
        }
        match self.mode {
            AppMode::Filter => return Vec::new(),
            AppMode::Settings { .. } => {
                return match ev.kind {
                    MouseEventKind::ScrollDown => vec![Message::ScrollSettings(1)],
                    MouseEventKind::ScrollUp => vec![Message::ScrollSettings(-1)],
                    _ => Vec::new(),
                }
            }
            AppMode::Normal => {}
        }

        let x = ev.column;
        let y = ev.row;

        // Identify which pane the cursor is over.
        let column_hit = if rect_contains(self.artist_rect, x, y) {
            Some((Column::Artists, self.artist_rect))
        } else if rect_contains(self.album_rect, x, y) {
            Some((Column::Albums, self.album_rect))
        } else if rect_contains(self.track_rect, x, y) {
            Some((Column::Tracks, self.track_rect))
        } else {
            None
        };
        let np_hit = rect_contains(self.np_rect, x, y);

        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((col, rect)) = column_hit {
                    // Determine if click is on a list row (inside the border).
                    if y > rect.y
                        && y + 1 < rect.y + rect.height
                        && x > rect.x
                        && x + 1 < rect.x + rect.width
                    {
                        let visible_row = (y - rect.y - 1) as usize;
                        let offset = match col {
                            Column::Artists => self.artist_state.offset(),
                            Column::Albums => self.album_state.offset(),
                            Column::Tracks => self.track_state.offset(),
                        };
                        let idx = offset + visible_row;
                        let len = match col {
                            Column::Artists => self.artist_filtered.len(),
                            Column::Albums => self.album_filtered.len(),
                            Column::Tracks => self.track_filtered.len(),
                        };
                        if idx < len {
                            return vec![Message::SelectAt(col, idx)];
                        }
                    }
                    return vec![Message::FocusColumn(col)];
                }
                Vec::new()
            }
            MouseEventKind::ScrollDown => {
                if let Some((col, _)) = column_hit {
                    return vec![Message::FocusColumn(col), Message::ScrollColumn(col, 1)];
                }
                if np_hit {
                    return vec![Message::ScrollMetaDown];
                }
                Vec::new()
            }
            MouseEventKind::ScrollUp => {
                if let Some((col, _)) = column_hit {
                    return vec![Message::FocusColumn(col), Message::ScrollColumn(col, -1)];
                }
                if np_hit {
                    return vec![Message::ScrollMetaUp];
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn map_filter_key(&self, key: KeyEvent) -> Option<Message> {
        match key.code {
            KeyCode::Esc => Some(Message::CancelFilter),
            KeyCode::Enter => Some(Message::ConfirmFilter),
            KeyCode::Backspace => Some(Message::FilterBackspace),
            KeyCode::Char(c) => Some(Message::FilterChar(c)),
            _ => None,
        }
    }
}
