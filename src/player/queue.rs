use crate::bandcamp::models::Track;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub track: Track,
    pub item_id: u64,
    pub album_title: String,
    pub artist_name: String,
    pub item_url: String,
    pub art_url: Option<String>,
    pub about: Option<String>,
    pub credits: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Debug)]
pub struct PlayQueue {
    pub items: Vec<QueueItem>,
    pub current: Option<usize>,
}

impl PlayQueue {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            current: None,
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

    /// Append items to the end of the queue. If the queue was empty, sets
    /// current to the first appended item.
    pub fn append_items(&mut self, items: Vec<QueueItem>) {
        if items.is_empty() {
            return;
        }
        if self.items.is_empty() {
            self.current = Some(0);
        }
        self.items.extend(items);
    }

    /// Insert items immediately after the current position. If the queue is
    /// empty, the items become the whole queue and current is set to 0.
    /// The current index is unchanged — it still points to the same item.
    pub fn insert_next_items(&mut self, items: Vec<QueueItem>) {
        if items.is_empty() {
            return;
        }
        let insert_at = self.current.map(|i| i + 1).unwrap_or(0);
        let tail = self.items.split_off(insert_at);
        self.items.extend(items);
        self.items.extend(tail);
        if self.current.is_none() {
            self.current = Some(0);
        }
    }

    pub fn next(&mut self) -> Option<&QueueItem> {
        let len = self.items.len();
        if len == 0 {
            return None;
        }

        if let Some(cur) = self.current {
            if cur + 1 < len {
                self.current = Some(cur + 1);
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
        if let Some(cur) = self.current
            && cur > 0
        {
            self.current = Some(cur - 1);
        }
        self.current_item()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bandcamp::models::Track;

    fn make_item(num: u32) -> QueueItem {
        QueueItem {
            track: Track {
                title: format!("Track {}", num),
                track_num: num,
                duration: 180.0,
                stream_url: None,
            },
            item_id: 1,
            album_title: "Album".to_string(),
            artist_name: "Artist".to_string(),
            item_url: "https://example.com/album".to_string(),
            art_url: None,
            about: None,
            credits: None,
            release_date: None,
        }
    }

    #[test]
    fn empty_queue() {
        let q = PlayQueue::new();
        assert!(q.current_item().is_none());
        assert_eq!(q.current, None);
    }

    #[test]
    fn replace_all_sets_current() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![make_item(1), make_item(2), make_item(3)], 1);
        assert_eq!(q.current, Some(1));
        assert_eq!(q.current_item().unwrap().track.track_num, 2);
    }

    #[test]
    fn replace_all_clamps_start_index() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![make_item(1)], 10);
        assert_eq!(q.current, Some(0));
    }

    #[test]
    fn replace_all_empty() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![], 0);
        assert_eq!(q.current, None);
    }

    #[test]
    fn next_advances() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![make_item(1), make_item(2), make_item(3)], 0);
        let item = q.next().unwrap();
        assert_eq!(item.track.track_num, 2);
        assert_eq!(q.current, Some(1));
    }

    #[test]
    fn next_at_end_returns_none() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![make_item(1), make_item(2)], 1);
        assert!(q.next().is_none());
        assert_eq!(q.current, Some(1));
    }

    #[test]
    fn prev_goes_back() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![make_item(1), make_item(2), make_item(3)], 2);
        let item = q.prev().unwrap();
        assert_eq!(item.track.track_num, 2);
    }

    #[test]
    fn prev_at_start_stays() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![make_item(1), make_item(2)], 0);
        let item = q.prev().unwrap();
        assert_eq!(item.track.track_num, 1);
        assert_eq!(q.current, Some(0));
    }

    #[test]
    fn next_on_empty_queue() {
        let mut q = PlayQueue::new();
        assert!(q.next().is_none());
    }

    #[test]
    fn prev_on_empty_queue() {
        let mut q = PlayQueue::new();
        assert!(q.prev().is_none());
    }

    // -- append_items --

    #[test]
    fn append_to_empty_sets_current() {
        let mut q = PlayQueue::new();
        q.append_items(vec![make_item(1), make_item(2)]);
        assert_eq!(q.current, Some(0));
        assert_eq!(q.items.len(), 2);
    }

    #[test]
    fn append_to_nonempty_preserves_current() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![make_item(1), make_item(2)], 1);
        q.append_items(vec![make_item(3), make_item(4)]);
        assert_eq!(q.current, Some(1)); // unchanged
        assert_eq!(q.items.len(), 4);
        assert_eq!(q.items[2].track.track_num, 3);
        assert_eq!(q.items[3].track.track_num, 4);
    }

    #[test]
    fn append_empty_vec_is_noop() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![make_item(1)], 0);
        q.append_items(vec![]);
        assert_eq!(q.current, Some(0));
        assert_eq!(q.items.len(), 1);
    }

    // -- insert_next_items --

    #[test]
    fn insert_next_into_empty_queue() {
        let mut q = PlayQueue::new();
        q.insert_next_items(vec![make_item(1), make_item(2)]);
        assert_eq!(q.current, Some(0));
        assert_eq!(q.items.len(), 2);
        assert_eq!(q.items[0].track.track_num, 1);
        assert_eq!(q.items[1].track.track_num, 2);
    }

    #[test]
    fn insert_next_in_middle() {
        let mut q = PlayQueue::new();
        // [1, 2*, 3, 4]  ->  insert [A, B]  ->  [1, 2*, A, B, 3, 4]
        q.replace_all(
            vec![make_item(1), make_item(2), make_item(3), make_item(4)],
            1,
        );
        q.insert_next_items(vec![make_item(10), make_item(11)]);
        assert_eq!(q.current, Some(1)); // still pointing at track 2
        assert_eq!(q.items.len(), 6);
        assert_eq!(q.items[1].track.track_num, 2);
        assert_eq!(q.items[2].track.track_num, 10);
        assert_eq!(q.items[3].track.track_num, 11);
        assert_eq!(q.items[4].track.track_num, 3);
        assert_eq!(q.items[5].track.track_num, 4);
    }

    #[test]
    fn insert_next_at_last_item() {
        let mut q = PlayQueue::new();
        // [1, 2*]  ->  insert [3]  ->  [1, 2*, 3]
        q.replace_all(vec![make_item(1), make_item(2)], 1);
        q.insert_next_items(vec![make_item(3)]);
        assert_eq!(q.current, Some(1));
        assert_eq!(q.items.len(), 3);
        assert_eq!(q.items[2].track.track_num, 3);
    }

    #[test]
    fn insert_next_empty_vec_is_noop() {
        let mut q = PlayQueue::new();
        q.replace_all(vec![make_item(1), make_item(2)], 0);
        q.insert_next_items(vec![]);
        assert_eq!(q.current, Some(0));
        assert_eq!(q.items.len(), 2);
    }

    #[test]
    fn next_plays_inserted_item() {
        let mut q = PlayQueue::new();
        // Insert next then advance — should land on the inserted track
        q.replace_all(vec![make_item(1), make_item(2)], 0);
        q.insert_next_items(vec![make_item(99)]);
        let item = q.next().unwrap();
        assert_eq!(item.track.track_num, 99);
        assert_eq!(q.current, Some(1));
    }
}
