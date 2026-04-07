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

/// Browsers we know how to read cookies from. Firefox-derived ones read
/// straight from the profile's SQLite db. Chromium-derived ones require the
/// macOS keychain entry for `<Browser> Safe Storage` and will prompt the user
/// once - that's expected and we only do it for the user's actual default.
#[derive(Debug, Clone, Copy)]
enum Browser {
    Firefox,
    Zen,
    LibreWolf,
    Waterfox,
    Chrome,
    Chromium,
    Brave,
    Edge,
    Vivaldi,
    Opera,
}

impl Browser {
    fn extract(self, domains: Vec<String>) -> Option<Vec<rookie::enums::Cookie>> {
        let d = Some(domains);
        match self {
            Browser::Firefox => rookie::firefox(d).ok(),
            Browser::Zen => rookie::zen(d).ok(),
            Browser::LibreWolf => rookie::librewolf(d).ok(),
            Browser::Waterfox => None, // rookie has no waterfox helper; falls through to path scan
            Browser::Chrome => rookie::chrome(d).ok(),
            Browser::Chromium => rookie::chromium(d).ok(),
            Browser::Brave => rookie::brave(d).ok(),
            Browser::Edge => rookie::edge(d).ok(),
            Browser::Vivaldi => rookie::vivaldi(d).ok(),
            Browser::Opera => rookie::opera(d).ok(),
        }
    }
}

pub fn extract_bandcamp_cookie() -> Result<Option<String>> {
    let domains = vec![".bandcamp.com".to_string(), "bandcamp.com".to_string()];

    // 1. Ask the OS which browser is the default and use that one directly.
    if let Some(browser) = detect_default_browser()
        && let Some(cookies) = browser.extract(domains.clone())
        && let Some(cookie) = find_identity_cookie(&cookies)
    {
        return Ok(Some(cookie));
    }

    // 2. Fall back to scanning known Firefox-based profile dirs (covers the
    //    case where the user signed into Bandcamp in a non-default browser,
    //    or when default-browser detection fails).
    let home = dirs::home_dir().unwrap_or_default();
    let firefox_based_paths = [
        home.join("Library/Application Support/Firefox"),
        home.join(".mozilla/firefox"),
        home.join("Library/Application Support/zen"),
        home.join(".zen"),
        home.join("Library/Application Support/librewolf"),
        home.join(".librewolf"),
        home.join("Library/Application Support/waterfox"),
        home.join(".waterfox"),
    ];

    for base_path in &firefox_based_paths {
        if !base_path.exists() {
            continue;
        }
        if let Some(cookie) = try_firefox_profiles(base_path, &domains)? {
            return Ok(Some(cookie));
        }
    }

    Ok(None)
}

#[cfg(target_os = "macos")]
fn detect_default_browser() -> Option<Browser> {
    let home = dirs::home_dir()?;
    let plist = home.join("Library/Preferences/com.apple.LaunchServices/com.apple.launchservices.secure.plist");
    let output = std::process::Command::new("plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(&plist)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let handlers = json.get("LSHandlers")?.as_array()?;
    let bundle_id = handlers.iter().find_map(|h| {
        if h.get("LSHandlerURLScheme")?.as_str()? == "http" {
            h.get("LSHandlerRoleAll")?.as_str().map(|s| s.to_lowercase())
        } else {
            None
        }
    })?;
    bundle_id_to_browser(&bundle_id)
}

#[cfg(target_os = "macos")]
fn bundle_id_to_browser(id: &str) -> Option<Browser> {
    match id {
        "org.mozilla.firefox" | "org.mozilla.firefoxdeveloperedition" | "org.mozilla.nightly" => {
            Some(Browser::Firefox)
        }
        "app.zen-browser.zen" | "org.zen-browser.zen" => Some(Browser::Zen),
        "io.gitlab.librewolf-community" => Some(Browser::LibreWolf),
        "net.waterfox.waterfox" | "org.waterfox.waterfox" => Some(Browser::Waterfox),
        "com.google.chrome" | "com.google.chrome.canary" => Some(Browser::Chrome),
        "org.chromium.chromium" => Some(Browser::Chromium),
        "com.brave.browser" | "com.brave.browser.nightly" => Some(Browser::Brave),
        "com.microsoft.edgemac" | "com.microsoft.edgemac.beta" | "com.microsoft.edgemac.dev" => {
            Some(Browser::Edge)
        }
        "com.vivaldi.vivaldi" => Some(Browser::Vivaldi),
        "com.operasoftware.opera" => Some(Browser::Opera),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn detect_default_browser() -> Option<Browser> {
    let output = std::process::Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let desktop = String::from_utf8(output.stdout).ok()?.trim().to_lowercase();
    desktop_file_to_browser(&desktop)
}

#[cfg(target_os = "linux")]
fn desktop_file_to_browser(desktop: &str) -> Option<Browser> {
    let stem = desktop.strip_suffix(".desktop").unwrap_or(desktop);
    match stem {
        "firefox" | "firefox-esr" | "firefox_firefox" => Some(Browser::Firefox),
        "zen" | "zen-browser" | "app.zen_browser.zen" => Some(Browser::Zen),
        "librewolf" | "io.gitlab.librewolf-community" => Some(Browser::LibreWolf),
        "waterfox" => Some(Browser::Waterfox),
        "google-chrome" | "google-chrome-stable" => Some(Browser::Chrome),
        "chromium" | "chromium-browser" => Some(Browser::Chromium),
        "brave-browser" | "brave" => Some(Browser::Brave),
        "microsoft-edge" | "microsoft-edge-stable" => Some(Browser::Edge),
        "vivaldi" | "vivaldi-stable" => Some(Browser::Vivaldi),
        "opera" => Some(Browser::Opera),
        _ => None,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn detect_default_browser() -> Option<Browser> {
    None
}

fn try_firefox_profiles(base_path: &std::path::Path, domains: &[String]) -> Result<Option<String>> {
    // Firefox-based browsers store profiles either directly under the base path
    // or nested under a "Profiles" subdirectory (e.g. Zen browser).
    let search_dirs = [base_path.to_path_buf(), base_path.join("Profiles")];

    for search_dir in &search_dirs {
        let Ok(entries) = std::fs::read_dir(search_dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let cookies_db = entry.path().join("cookies.sqlite");
            if cookies_db.exists()
                && let Ok(cookies) =
                    rookie::firefox_based(cookies_db, Some(domains.to_vec()))
                && let Some(cookie) = find_identity_cookie(&cookies)
            {
                return Ok(Some(cookie));
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
