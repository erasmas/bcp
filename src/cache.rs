use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::bandcamp::models::Album;
use crate::config;

#[derive(Debug, Serialize, Deserialize)]
struct CachedCollection {
    albums: Vec<Album>,
    timestamp: u64,
}

fn cache_file() -> Result<PathBuf> {
    Ok(config::cache_dir()?.join("collection.json"))
}

pub fn load_cached_collection() -> Result<Option<Vec<Album>>> {
    let path = cache_file()?;
    if !path.exists() {
        return Ok(None);
    }

    let data = std::fs::read_to_string(&path)?;
    let cached: CachedCollection = serde_json::from_str(&data)?;

    Ok(Some(cached.albums))
}

pub fn save_collection_cache(albums: &[Album]) -> Result<()> {
    let path = cache_file()?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let cached = CachedCollection {
        albums: albums.to_vec(),
        timestamp: now,
    };

    let data = serde_json::to_string(&cached)?;
    std::fs::write(&path, data)?;
    Ok(())
}

pub fn invalidate_cache() -> Result<()> {
    let path = cache_file()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}
