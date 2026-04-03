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

/// Returns the library directory for downloaded tracks.
/// Reads from ~/.config/bcp/config.toml [library] path if set,
/// otherwise defaults to ~/Music/bcp/.
pub fn library_dir() -> Result<PathBuf> {
    // Check config.toml for custom path
    let config_path = config_dir()?.join("config.toml");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("path") {
                if let Some(val) = trimmed.split('=').nth(1) {
                    let val = val.trim().trim_matches('"').trim_matches('\'');
                    if !val.is_empty() {
                        let dir = PathBuf::from(shellexpand(val));
                        std::fs::create_dir_all(&dir)?;
                        return Ok(dir);
                    }
                }
            }
        }
    }

    // Default: ~/Music/bcp/
    let dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join("Music")
        .join("bcp");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn shellexpand(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), &path[2..]);
        }
    }
    path.to_string()
}
