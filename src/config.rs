use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Default, Deserialize)]
struct Config {
    library: Option<LibraryConfig>,
}

#[derive(Debug, Deserialize)]
struct LibraryConfig {
    path: Option<String>,
    format: Option<String>,
}

/// Supported download formats with descriptions.
pub const DOWNLOAD_FORMATS: &[(&str, &str)] = &[
    ("flac", "FLAC (lossless)"),
    ("wav", "WAV (lossless)"),
    ("aiff-lossless", "AIFF (lossless)"),
    ("alac", "ALAC (lossless)"),
    ("mp3-320", "MP3 320kbps"),
    ("mp3-v0", "MP3 V0 (VBR)"),
    ("aac-hi", "AAC"),
    ("vorbis", "Ogg Vorbis"),
];

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

/// Returns the configured download format (default: "flac").
/// The result is cached for the lifetime of the process.
pub fn download_format() -> String {
    static FORMAT: OnceLock<String> = OnceLock::new();
    FORMAT
        .get_or_init(|| {
            let config_path = match config_dir() {
                Ok(d) => d.join("config.toml"),
                Err(_) => return "flac".to_string(),
            };
            if config_path.exists()
                && let Ok(content) = std::fs::read_to_string(&config_path)
                && let Ok(config) = toml::from_str::<Config>(&content)
                && let Some(lib) = config.library
                && let Some(fmt) = lib.format
                && !fmt.is_empty()
                && DOWNLOAD_FORMATS.iter().any(|(k, _)| *k == fmt)
            {
                return fmt;
            }
            "flac".to_string()
        })
        .clone()
}

/// Returns the file extension for a download format.
pub fn format_extension(format: &str) -> &str {
    match format {
        "flac" => "flac",
        "wav" => "wav",
        "aiff-lossless" => "aiff",
        "alac" => "m4a",
        "mp3-320" | "mp3-v0" => "mp3",
        "aac-hi" => "m4a",
        "vorbis" => "ogg",
        _ => "flac",
    }
}

/// Returns the description for a download format.
pub fn format_description(format: &str) -> &str {
    DOWNLOAD_FORMATS
        .iter()
        .find(|(k, _)| *k == format)
        .map(|(_, desc)| *desc)
        .unwrap_or("FLAC (lossless)")
}

fn shellexpand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return format!("{}/{}", home.display(), rest);
    }
    path.to_string()
}
