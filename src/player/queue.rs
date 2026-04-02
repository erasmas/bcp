use crate::bandcamp::models::Track;

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub track: Track,
    pub album_title: String,
    pub artist_name: String,
    pub art_url: Option<String>,
}

#[derive(Debug)]
pub struct PlayQueue {
    pub items: Vec<QueueItem>,
    pub current: Option<usize>,
    pub shuffle: bool,
    pub repeat: bool,
}

impl PlayQueue {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            current: None,
            shuffle: false,
            repeat: false,
        }
    }

    pub fn current_item(&self) -> Option<&QueueItem> {
        self.current.and_then(|i| self.items.get(i))
    }

    pub fn replace_all(&mut self, items: Vec<QueueItem>, start_index: usize) {
        self.items = items;
        self.current = if self.items.is_empty() {
            None
        } else {
            Some(start_index.min(self.items.len().saturating_sub(1)))
        };
    }

    pub fn next(&mut self) -> Option<&QueueItem> {
        let len = self.items.len();
        if len == 0 {
            return None;
        }

        if self.shuffle {
            use std::time::{SystemTime, UNIX_EPOCH};
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize;
            self.current = Some(seed % len);
        } else if let Some(cur) = self.current {
            if cur + 1 < len {
                self.current = Some(cur + 1);
            } else if self.repeat {
                self.current = Some(0);
            } else {
                return None;
            }
        }

        self.current_item()
    }

    pub fn prev(&mut self) -> Option<&QueueItem> {
        if self.items.is_empty() {
            return None;
        }
        if let Some(cur) = self.current {
            if cur > 0 {
                self.current = Some(cur - 1);
            } else if self.repeat {
                self.current = Some(self.items.len() - 1);
            }
        }
        self.current_item()
    }

}
