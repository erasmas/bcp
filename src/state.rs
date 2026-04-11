use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::app::Column;
use crate::config;
use crate::player::queue::QueueItem;

#[derive(Serialize, Deserialize)]
pub struct AppState {
    // Playback
    pub queue_items: Vec<QueueItem>,
    pub queue_current: Option<usize>,
    pub is_paused: bool,
    pub elapsed: f64,
    // UI
    pub focus: Column,
    pub artist_selected: Option<usize>,
    pub artist_offset: usize,
    pub album_selected: Option<usize>,
    pub album_offset: usize,
    pub track_selected: Option<usize>,
    pub track_offset: usize,
    pub meta_scroll: usize,
    #[serde(default)]
    pub queue_visible: bool,
    #[serde(default)]
    pub queue_selected: Option<usize>,
    #[serde(default)]
    pub queue_offset: usize,
}

fn state_file() -> Result<PathBuf> {
    Ok(config::cache_dir()?.join("state.json"))
}

pub fn load_state() -> Result<Option<AppState>> {
    let path = state_file()?;
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(&path)?;
    let state: AppState = serde_json::from_str(&data)?;
    Ok(Some(state))
}

pub fn save_state(state: &AppState) -> Result<()> {
    let path = state_file()?;
    let data = serde_json::to_string(state)?;
    std::fs::write(&path, data)?;
    Ok(())
}
