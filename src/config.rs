use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    ("mp3-320", "MP3 320kbps"),
    ("mp3-v0", "MP3 V0 (VBR)"),
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

static FORMAT_INDEX: AtomicUsize = AtomicUsize::new(0);
static FORMAT_INITIALIZED: OnceLock<()> = OnceLock::new();

fn init_format_index() {
    FORMAT_INITIALIZED.get_or_init(|| {
        let config_path = match config_dir() {
            Ok(d) => d.join("config.toml"),
            Err(_) => return,
        };
        if config_path.exists()
            && let Ok(content) = std::fs::read_to_string(&config_path)
            && let Ok(config) = toml::from_str::<Config>(&content)
            && let Some(lib) = config.library
            && let Some(fmt) = lib.format
            && !fmt.is_empty()
            && let Some(idx) = DOWNLOAD_FORMATS.iter().position(|(k, _)| *k == fmt)
        {
            FORMAT_INDEX.store(idx, Ordering::Relaxed);
        }
    });
}

/// Returns the configured download format (default: "flac").
pub fn download_format() -> String {
    init_format_index();
    let idx = FORMAT_INDEX.load(Ordering::Relaxed);
    DOWNLOAD_FORMATS[idx].0.to_string()
}

/// Cycle the download format by delta (+1 for next, -1 for previous).
/// Persists the change to config.toml.
pub fn cycle_download_format(delta: i8) {
    init_format_index();
    let len = DOWNLOAD_FORMATS.len();
    let old = FORMAT_INDEX.load(Ordering::Relaxed);
    let new = (old as isize + delta as isize).rem_euclid(len as isize) as usize;
    FORMAT_INDEX.store(new, Ordering::Relaxed);
    save_format(DOWNLOAD_FORMATS[new].0);
}

fn save_format(format: &str) {
    let Ok(dir) = config_dir() else { return };
    let path = dir.join("config.toml");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc = content.parse::<toml::Table>().unwrap_or_default();
    let library = doc
        .entry("library")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    if let toml::Value::Table(lib) = library {
        lib.insert(
            "format".to_string(),
            toml::Value::String(format.to_string()),
        );
    }
    let _ = std::fs::write(&path, doc.to_string());
}

/// Returns the file extension for a download format.
pub fn format_extension(format: &str) -> &str {
    match format {
        "flac" => "flac",
        "mp3-320" | "mp3-v0" => "mp3",
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
