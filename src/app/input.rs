use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, AppScreen, LoginStep, Message};

impl App {
    /// Pure mapping from key event to message. No state mutations.
    pub(crate) fn map_key(&self, key: KeyEvent) -> Option<Message> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Some(Message::Quit);
        }

        match self.screen {
            AppScreen::Login => self.map_login_key(key),
            AppScreen::Loading => None,
            AppScreen::Main => {
                if self.show_settings {
                    return match key.code {
                        KeyCode::Esc | KeyCode::Char('?') => Some(Message::ToggleSettings),
                        KeyCode::Char('q') => Some(Message::Quit),
                        _ => None,
                    };
                }
                if self.filter_mode {
                    let is_nav = matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Enter);
                    if !is_nav {
                        return self.map_filter_key(key);
                    }
                    // Enter during filter: confirm filter then fall through to main
                    if key.code == KeyCode::Enter {
                        return Some(Message::ConfirmFilter);
                    }
                }
                self.map_main_key(key)
            }
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
            KeyCode::Char('h') | KeyCode::Left => Some(Message::FocusLeft),
            KeyCode::Char('l') | KeyCode::Right => Some(Message::FocusRight),
            KeyCode::Char('?') => Some(Message::ToggleSettings),
            KeyCode::Char('d') => Some(Message::Download),
            KeyCode::Char('D') => Some(Message::DownloadAll),
            KeyCode::Char('j') | KeyCode::Down => Some(Message::MoveDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Message::MoveUp),
            KeyCode::Char('g') => Some(Message::MoveToTop),
            KeyCode::Char('G') => Some(Message::MoveToBottom),
            KeyCode::Enter => Some(Message::Enter),
            KeyCode::Char(' ') => Some(Message::TogglePause),
            KeyCode::Char('n') => Some(Message::NextTrack),
            KeyCode::Char('p') => Some(Message::PrevTrack),
            KeyCode::Char('J') => Some(Message::ScrollMetaDown),
            KeyCode::Char('K') => Some(Message::ScrollMetaUp),
            KeyCode::Tab => Some(Message::ToggleMetaScroll),
            KeyCode::Char('r') => Some(Message::Refresh),
            KeyCode::Char('/') => Some(Message::StartFilter),
            _ => None,
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
