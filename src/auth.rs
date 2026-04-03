use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::bandcamp::models::AuthData;
use crate::config;

fn auth_file() -> Result<PathBuf> {
    Ok(config::config_dir()?.join("auth.json"))
}

pub fn load_auth() -> Result<Option<AuthData>> {
    let path = auth_file()?;
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(&path)?;
    let auth: AuthData = serde_json::from_str(&data)?;
    Ok(Some(auth))
}

pub fn save_auth(auth: &AuthData) -> Result<()> {
    let path = auth_file()?;
    let data = serde_json::to_string_pretty(auth)?;
    std::fs::write(&path, data)?;
    Ok(())
}

pub fn extract_bandcamp_cookie() -> Result<Option<String>> {
    let domains = vec![
        ".bandcamp.com".to_string(),
        "bandcamp.com".to_string(),
    ];

    // Try Firefox-based browsers via custom profile paths
    // (skip rookie::firefox/chrome/safari to avoid macOS Keychain prompts)
    let home = dirs::home_dir().unwrap_or_default();
    let firefox_based_paths = [
        home.join("Library/Application Support/Firefox"),        // Firefox macOS
        home.join(".mozilla/firefox"),                           // Firefox Linux
        home.join("Library/Application Support/zen"),            // Zen macOS
        home.join(".zen"),                                       // Zen Linux
        home.join("Library/Application Support/librewolf"),      // LibreWolf macOS
        home.join(".librewolf"),                                 // LibreWolf Linux
        home.join("Library/Application Support/waterfox"),       // Waterfox macOS
        home.join(".waterfox"),                                  // Waterfox Linux
    ];

    for base_path in &firefox_based_paths {
        if !base_path.exists() {
            continue;
        }
        eprintln!("Trying Firefox-based browser at {:?}", base_path);
        if let Some(cookie) = try_firefox_profiles(base_path, &domains)? {
            eprintln!("Found identity cookie in {:?}", base_path);
            return Ok(Some(cookie));
        }
    }

    Ok(None)
}

fn try_firefox_profiles(base_path: &std::path::Path, domains: &[String]) -> Result<Option<String>> {
    // Firefox-based browsers store profiles either directly under the base path
    // or nested under a "Profiles" subdirectory (e.g. Zen browser)
    let search_dirs = [
        base_path.to_path_buf(),
        base_path.join("Profiles"),
    ];

    for search_dir in &search_dirs {
        let Ok(entries) = std::fs::read_dir(search_dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let cookies_db = entry.path().join("cookies.sqlite");
            if cookies_db.exists() {
                let db_path = cookies_db.to_string_lossy().to_string();
                match rookie::any_browser(&db_path, Some(domains.to_vec()), None) {
                    Ok(cookies) => {
                        if let Some(cookie) = find_identity_cookie(&cookies) {
                            return Ok(Some(cookie));
                        }
                    }
                    Err(e) => {
                        eprintln!("  Profile {:?}: {}", entry.path(), e);
                    }
                }
            }
        }
    }

    Ok(None)
}

fn find_identity_cookie(cookies: &[rookie::enums::Cookie]) -> Option<String> {
    cookies
        .iter()
        .find(|c| c.name == "identity" && !c.value.is_empty())
        .map(|c| c.value.clone())
}

pub fn open_login_page() -> Result<()> {
    open::that("https://bandcamp.com/login")
        .context("Failed to open browser. Please open https://bandcamp.com/login manually.")?;
    Ok(())
}

pub fn clear_auth() -> Result<()> {
    let path = auth_file()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}
