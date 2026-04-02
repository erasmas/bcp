use anyhow::Result;
use std::path::PathBuf;

pub fn config_dir() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
        .join("bcp");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn cache_dir() -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?
        .join("bcp");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

