use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
struct Config {
    library: Option<LibraryConfig>,
}

#[derive(Debug, Deserialize)]
struct LibraryConfig {
    path: Option<String>,
}

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
    let config_path = config_dir()?.join("config.toml");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content).unwrap_or_default();
        if let Some(lib) = config.library
            && let Some(path) = lib.path
            && !path.is_empty()
        {
            let dir = PathBuf::from(shellexpand(&path));
            std::fs::create_dir_all(&dir)?;
            return Ok(dir);
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
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return format!("{}/{}", home.display(), rest);
    }
    path.to_string()
}
