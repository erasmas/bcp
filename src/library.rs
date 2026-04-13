use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::bandcamp::models::Album;
use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryIndex {
    pub albums: HashMap<u64, DownloadedAlbum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadedAlbum {
    pub item_id: u64,
    pub album_title: String,
    pub artist_name: String,
    pub tracks: Vec<DownloadedTrack>,
    pub status: AlbumDownloadStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadedTrack {
    pub track_num: u32,
    pub title: String,
    pub file_name: String,
    pub downloaded: bool,
    #[serde(default)]
    pub duration: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AlbumDownloadStatus {
    Complete,
    Partial,
    Downloading,
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Progress {
        item_id: u64,
        downloaded: u64,
        total: Option<u64>,
    },
    TrackDone {
        item_id: u64,
        track_num: u32,
    },
    AlbumDone {
        item_id: u64,
    },
    Error {
        item_id: u64,
        msg: String,
    },
}

impl LibraryIndex {
    pub fn new() -> Self {
        Self {
            albums: HashMap::new(),
        }
    }

    fn index_path() -> Result<PathBuf> {
        Ok(config::library_dir()?.join("library.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::index_path()?;
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = std::fs::read_to_string(&path)?;
        let index: Self = serde_json::from_str(&data).context("Failed to parse library index")?;
        Ok(index)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::index_path()?;
        let data = serde_json::to_string_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Get the local file path for a track, if it has been downloaded.
    /// Checks multiple extensions since tracks may have been downloaded in different formats.
    pub fn track_path(&self, item_id: u64, track_num: u32) -> Option<PathBuf> {
        let album = self.albums.get(&item_id)?;
        let track = album.tracks.iter().find(|t| t.track_num == track_num)?;
        if !track.downloaded {
            return None;
        }
        let base = config::library_dir().ok()?;
        let dir = album_dir(&base, &album.artist_name, &album.album_title);

        // Check the stored filename first
        let path = dir.join(&track.file_name);
        if path.exists() {
            return Some(path);
        }

        // Check other extensions (format may have changed since download)
        let exts = ["flac", "mp3", "wav", "aiff", "m4a", "ogg"];
        let stem = track
            .file_name
            .rsplit_once('.')
            .map(|(s, _)| s)
            .unwrap_or(&track.file_name);
        for ext in exts {
            let alt = dir.join(format!("{}.{}", stem, ext));
            if alt.exists() {
                return Some(alt);
            }
        }

        None
    }

    /// Check if an album is fully downloaded.
    pub fn is_downloaded(&self, item_id: u64) -> bool {
        self.albums
            .get(&item_id)
            .is_some_and(|a| a.status == AlbumDownloadStatus::Complete)
    }

    /// Check download status of an album.
    pub fn album_status(&self, item_id: u64) -> Option<AlbumDownloadStatus> {
        self.albums.get(&item_id).map(|a| a.status)
    }

    /// Check if a specific track is downloaded.
    pub fn is_track_downloaded(&self, item_id: u64, track_num: u32) -> bool {
        self.albums
            .get(&item_id)
            .and_then(|a| a.tracks.iter().find(|t| t.track_num == track_num))
            .is_some_and(|t| t.downloaded)
    }

    /// Get download progress as (downloaded, total) track counts.
    #[allow(dead_code)]
    pub fn progress(&self, item_id: u64) -> (usize, usize) {
        match self.albums.get(&item_id) {
            Some(album) => {
                let done = album.tracks.iter().filter(|t| t.downloaded).count();
                (done, album.tracks.len())
            }
            None => (0, 0),
        }
    }
}

fn album_dir(base: &std::path::Path, artist: &str, title: &str) -> PathBuf {
    base.join(sanitize_filename(artist))
        .join(sanitize_filename(title))
}

/// On-disk album metadata stored as metadata.json in the album directory.
#[derive(Debug, Serialize, Deserialize)]
pub struct AlbumMetadata {
    pub item_id: u64,
    pub tracks: Vec<TrackMetadata>,
    pub about: Option<String>,
    pub credits: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackMetadata {
    pub track_num: u32,
    pub title: String,
    pub duration: f64,
    pub stream_url: Option<String>,
}

/// Download cover art to the album's directory if not already present.
/// Returns the path to the cover file.
pub async fn download_cover(artist: &str, album_title: &str, art_url: &str) -> Result<PathBuf> {
    let base = config::library_dir()?;
    let dir = album_dir(&base, artist, album_title);
    std::fs::create_dir_all(&dir)?;
    let cover_path = dir.join("cover.jpg");
    if cover_path.exists() {
        return Ok(cover_path);
    }
    let art_sized = art_url.replace("_16.", "_5.");
    let client = reqwest::Client::new();
    let resp = client.get(&art_sized).send().await?;
    let bytes = resp.bytes().await?;
    std::fs::write(&cover_path, &bytes)?;
    Ok(cover_path)
}

/// Check if cover art exists for an album.
pub fn has_cover(artist: &str, album_title: &str) -> bool {
    config::library_dir()
        .map(|base| {
            album_dir(&base, artist, album_title)
                .join("cover.jpg")
                .exists()
        })
        .unwrap_or(false)
}

/// Save album metadata to metadata.json in the album's directory.
pub fn save_album_metadata(artist: &str, album_title: &str, meta: &AlbumMetadata) -> Result<()> {
    let base = config::library_dir()?;
    let dir = album_dir(&base, artist, album_title);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("metadata.json");
    let data = serde_json::to_string_pretty(meta)?;
    std::fs::write(&path, data)?;
    Ok(())
}

/// Load album metadata from metadata.json in the album's directory.
pub fn load_album_metadata(artist: &str, album_title: &str) -> Option<AlbumMetadata> {
    let base = config::library_dir().ok()?;
    let dir = album_dir(&base, artist, album_title);
    let path = dir.join("metadata.json");
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn sanitize_for_path(name: &str) -> String {
    sanitize_filename(name)
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn track_filename(track_num: u32, title: &str, ext: &str) -> String {
    format!("{:02} - {}.{}", track_num, sanitize_filename(title), ext)
}

/// Spawn a background task to download an album's tracks in the configured format (e.g. FLAC).
pub fn download_album(
    album: &Album,
    identity_cookie: &str,
    download_page_url: Option<String>,
) -> Result<mpsc::UnboundedReceiver<DownloadEvent>> {
    let (tx, rx) = mpsc::unbounded_channel();
    let base = config::library_dir()?;
    let dir = album_dir(&base, &album.artist_name, &album.album_title);
    std::fs::create_dir_all(&dir)?;

    let item_id = album.item_id;
    let tracks: Vec<_> = album
        .tracks
        .iter()
        .map(|t| (t.track_num, t.title.clone(), t.stream_url.clone()))
        .collect();
    let artist = album.artist_name.clone();
    let album_title = album.album_title.clone();
    let art_url = album.art_url.clone();
    let cookie = identity_cookie.to_string();
    let preferred_format = config::download_format();

    tokio::spawn(async move {
        let client = reqwest::Client::new();

        // Download cover art
        if let Some(ref art) = art_url {
            let _ = download_cover(&artist, &album_title, art).await;
        }

        // Try HQ download via download page first
        let mut hq_ok = false;
        if let Some(dl_url) = download_page_url
            && let Some((url, fmt_key)) =
                fetch_hq_download_url(&client, &cookie, &dl_url, &preferred_format).await
        {
            let ctx = HqDownloadCtx {
                client: &client,
                cookie: &cookie,
                dir: &dir,
                item_id,
                tx: &tx,
            };
            let hq_tracks: Vec<_> = tracks.iter().map(|(n, t, _)| (*n, t.clone())).collect();
            download_and_extract_hq(&ctx, &url, &fmt_key, &hq_tracks).await;
            hq_ok = true;
        }

        // Fall back to stream URLs (mp3-128) for non-purchased or failed HQ downloads
        if !hq_ok {
            for (track_num, title, stream_url) in &tracks {
                let Some(url) = stream_url else {
                    let _ = tx.send(DownloadEvent::Error {
                        item_id,
                        msg: format!("No stream URL for track {}", track_num),
                    });
                    continue;
                };

                let file_name = track_filename(*track_num, title, "mp3");
                let file_path = dir.join(&file_name);

                if file_path.exists() {
                    let _ = tx.send(DownloadEvent::TrackDone {
                        item_id,
                        track_num: *track_num,
                    });
                    continue;
                }

                let resp = client
                    .get(url)
                    .header("Cookie", format!("identity={}", cookie))
                    .send()
                    .await;

                match resp {
                    Ok(resp) => match resp.bytes().await {
                        Ok(bytes) => {
                            if let Err(e) = std::fs::write(&file_path, &bytes) {
                                let _ = tx.send(DownloadEvent::Error {
                                    item_id,
                                    msg: format!("Write error: {}", e),
                                });
                                continue;
                            }
                            let _ = tx.send(DownloadEvent::TrackDone {
                                item_id,
                                track_num: *track_num,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(DownloadEvent::Error {
                                item_id,
                                msg: format!("Download error: {}", e),
                            });
                        }
                    },
                    Err(e) => {
                        let _ = tx.send(DownloadEvent::Error {
                            item_id,
                            msg: format!("Request error: {}", e),
                        });
                    }
                }
            }
        }

        let _ = tx.send(DownloadEvent::AlbumDone { item_id });
    });

    Ok(rx)
}

/// Fetch the HQ download URL from a Bandcamp download page.
/// Returns (url, format_key) for the best available format.
async fn fetch_hq_download_url(
    client: &reqwest::Client,
    cookie: &str,
    download_page_url: &str,
    preferred_format: &str,
) -> Option<(String, String)> {
    let resp = client
        .get(download_page_url)
        .header("Cookie", format!("identity={}", cookie))
        .send()
        .await
        .ok()?;
    let html = resp.text().await.ok()?;
    let formats = crate::bandcamp::client::extract_download_formats_from_html(&html).ok()?;

    if let Some(dl) = formats.formats.get(preferred_format) {
        return Some((dl.url.clone(), preferred_format.to_string()));
    }
    for (fmt, _) in config::DOWNLOAD_FORMATS {
        if let Some(dl) = formats.formats.get(*fmt) {
            return Some((dl.url.clone(), fmt.to_string()));
        }
    }
    None
}

struct HqDownloadCtx<'a> {
    client: &'a reqwest::Client,
    cookie: &'a str,
    dir: &'a std::path::Path,
    item_id: u64,
    tx: &'a tokio::sync::mpsc::UnboundedSender<DownloadEvent>,
}

/// Download a HQ album file (usually a ZIP), streaming to a temp file, then extract tracks.
async fn download_and_extract_hq(
    ctx: &HqDownloadCtx<'_>,
    url: &str,
    fmt_key: &str,
    tracks: &[(u32, String)],
) {
    use tokio::io::AsyncWriteExt;

    let item_id = ctx.item_id;
    let ext = config::format_extension(fmt_key);

    let resp = match ctx
        .client
        .get(url)
        .header("Cookie", format!("identity={}", ctx.cookie))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let _ = ctx.tx.send(DownloadEvent::Error {
                item_id,
                msg: format!("Download failed: HTTP {}", r.status()),
            });
            return;
        }
        Err(e) => {
            let _ = ctx.tx.send(DownloadEvent::Error {
                item_id,
                msg: format!("Download request error: {}", e),
            });
            return;
        }
    };

    // Stream response body to a temp file with progress reporting
    let total = resp.content_length();
    let tmp_path = ctx.dir.join(format!(".download-{}.tmp", item_id));
    let mut tmp_file = match tokio::fs::File::create(&tmp_path).await {
        Ok(f) => f,
        Err(e) => {
            let _ = ctx.tx.send(DownloadEvent::Error {
                item_id,
                msg: format!("Failed to create temp file: {}", e),
            });
            return;
        }
    };

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                if let Err(e) = tmp_file.write_all(&bytes).await {
                    let _ = ctx.tx.send(DownloadEvent::Error {
                        item_id,
                        msg: format!("Write error: {}", e),
                    });
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    return;
                }
                downloaded += bytes.len() as u64;
                let _ = ctx.tx.send(DownloadEvent::Progress {
                    item_id,
                    downloaded,
                    total,
                });
            }
            Err(e) => {
                let _ = ctx.tx.send(DownloadEvent::Error {
                    item_id,
                    msg: format!("Download stream error: {}", e),
                });
                let _ = tokio::fs::remove_file(&tmp_path).await;
                return;
            }
        }
    }
    drop(tmp_file);

    // Extract on a blocking thread to avoid starving the tokio executor
    let tmp_path_clone = tmp_path.clone();
    let dir_clone = ctx.dir.to_path_buf();
    let ext_owned = ext.to_string();
    let tracks_owned = tracks.to_vec();
    let tx_clone = ctx.tx.clone();

    let extract_result = tokio::task::spawn_blocking(move || {
        extract_downloaded_file(
            &tmp_path_clone,
            &dir_clone,
            &ext_owned,
            &tracks_owned,
            item_id,
            &tx_clone,
        )
    })
    .await;

    if let Err(e) = extract_result {
        let _ = ctx.tx.send(DownloadEvent::Error {
            item_id,
            msg: format!("Extraction task failed: {}", e),
        });
    }

    // Clean up temp file
    let _ = tokio::fs::remove_file(&tmp_path).await;
}

/// Extract tracks from a downloaded file (ZIP or raw audio). Runs on a blocking thread.
fn extract_downloaded_file(
    tmp_path: &std::path::Path,
    dir: &std::path::Path,
    ext: &str,
    tracks: &[(u32, String)],
    item_id: u64,
    tx: &tokio::sync::mpsc::UnboundedSender<DownloadEvent>,
) {
    let file = match std::fs::File::open(tmp_path) {
        Ok(f) => f,
        Err(e) => {
            let _ = tx.send(DownloadEvent::Error {
                item_id,
                msg: format!("Failed to open download: {}", e),
            });
            return;
        }
    };

    if let Ok(mut archive) = zip::ZipArchive::new(&file) {
        // ZIP archive - extract individual tracks
        for i in 0..archive.len() {
            let mut entry = match archive.by_index(i) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let zip_name = entry.name().to_string();

            if entry.is_dir() || zip_name.ends_with('/') {
                continue;
            }

            if let Some((track_num, title)) = match_zip_entry_to_track(&zip_name, tracks) {
                let out_name = track_filename(track_num, &title, ext);
                let out_path = dir.join(&out_name);
                if out_path.exists() {
                    let _ = tx.send(DownloadEvent::TrackDone { item_id, track_num });
                    continue;
                }

                let mut out_file = match std::fs::File::create(&out_path) {
                    Ok(f) => f,
                    Err(e) => {
                        let _ = tx.send(DownloadEvent::Error {
                            item_id,
                            msg: format!("Failed to create file for track {}: {}", track_num, e),
                        });
                        continue;
                    }
                };
                match std::io::copy(&mut entry, &mut out_file) {
                    Ok(_) => {
                        let _ = tx.send(DownloadEvent::TrackDone { item_id, track_num });
                    }
                    Err(e) => {
                        let _ = tx.send(DownloadEvent::Error {
                            item_id,
                            msg: format!("Failed to extract track {}: {}", track_num, e),
                        });
                    }
                }
            }
        }
    } else {
        // Raw audio file (single track purchase) - just rename the temp file
        if let Some((track_num, title)) = tracks.first() {
            let out_name = track_filename(*track_num, title, ext);
            let out_path = dir.join(&out_name);
            if std::fs::rename(tmp_path, &out_path).is_ok() {
                let _ = tx.send(DownloadEvent::TrackDone {
                    item_id,
                    track_num: *track_num,
                });
            }
        }
    }
}

/// Match a ZIP archive entry name to a track by looking for the track number prefix.
/// Bandcamp names files like "Artist - Album - 01 Title.flac".
fn match_zip_entry_to_track(zip_name: &str, tracks: &[(u32, String)]) -> Option<(u32, String)> {
    // Get the filename without directory
    let file_name = zip_name.rsplit('/').next().unwrap_or(zip_name);

    // Extract the part after the last " - " separator, which is where Bandcamp
    // puts "01 Title.flac". Fall back to the full filename.
    let track_part = file_name
        .rfind(" - ")
        .map(|pos| &file_name[pos + 3..])
        .unwrap_or(file_name);

    // Match track number at the start of the track part
    for (track_num, title) in tracks {
        let num_str = format!("{:02}", track_num);
        if track_part.starts_with(&format!("{} ", num_str))
            || track_part.starts_with(&format!("{}.", num_str))
            || track_part.starts_with(&format!("{}-", num_str))
            || track_part.starts_with(&format!("{}_", num_str))
        {
            return Some((*track_num, title.clone()));
        }
    }

    None
}

/// Prepare the library index entry for an album that's about to be downloaded.
pub fn prepare_album_entry(album: &Album) -> DownloadedAlbum {
    let fmt = config::download_format();
    let ext = config::format_extension(&fmt);
    let tracks = album
        .tracks
        .iter()
        .map(|t| DownloadedTrack {
            track_num: t.track_num,
            title: t.title.clone(),
            file_name: track_filename(t.track_num, &t.title, ext),
            downloaded: false,
            duration: t.duration,
        })
        .collect();

    DownloadedAlbum {
        item_id: album.item_id,
        album_title: album.album_title.clone(),
        artist_name: album.artist_name.clone(),
        tracks,
        status: AlbumDownloadStatus::Downloading,
    }
}
