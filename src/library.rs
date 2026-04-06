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
    TrackDone { item_id: u64, track_num: u32 },
    AlbumDone { item_id: u64 },
    Error { item_id: u64, msg: String },
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
    pub fn track_path(&self, item_id: u64, track_num: u32) -> Option<PathBuf> {
        let album = self.albums.get(&item_id)?;
        let track = album.tracks.iter().find(|t| t.track_num == track_num)?;
        if !track.downloaded {
            return None;
        }
        let base = config::library_dir().ok()?;
        let dir = album_dir(&base, &album.artist_name, &album.album_title);
        let path = dir.join(&track.file_name);
        if path.exists() { Some(path) } else { None }
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

fn track_filename(track_num: u32, title: &str) -> String {
    format!("{:02} - {}.mp3", track_num, sanitize_filename(title))
}

/// Spawn a background task to download an album's tracks.
/// Returns a receiver for progress events.
pub fn download_album(
    album: &Album,
    identity_cookie: &str,
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

    tokio::spawn(async move {
        let client = reqwest::Client::new();

        // Download cover art
        if let Some(ref art) = art_url {
            let _ = download_cover(&artist, &album_title, art).await;
        }

        // Download tracks sequentially
        for (track_num, title, stream_url) in &tracks {
            let Some(url) = stream_url else {
                let _ = tx.send(DownloadEvent::Error {
                    item_id,
                    msg: format!("No stream URL for track {}", track_num),
                });
                continue;
            };

            let file_name = track_filename(*track_num, title);
            let file_path = dir.join(&file_name);

            // Skip if already exists
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

        let _ = tx.send(DownloadEvent::AlbumDone { item_id });
    });

    Ok(rx)
}

/// Spawn a background task to download a single track from an album.
pub fn download_track(
    album: &Album,
    track_num: u32,
    identity_cookie: &str,
) -> Result<mpsc::UnboundedReceiver<DownloadEvent>> {
    let (tx, rx) = mpsc::unbounded_channel();
    let base = config::library_dir()?;
    let dir = album_dir(&base, &album.artist_name, &album.album_title);
    std::fs::create_dir_all(&dir)?;

    let item_id = album.item_id;
    let track = album
        .tracks
        .iter()
        .find(|t| t.track_num == track_num)
        .context("Track not found")?;
    let title = track.title.clone();
    let stream_url = track.stream_url.clone();
    let cookie = identity_cookie.to_string();

    tokio::spawn(async move {
        let Some(url) = stream_url else {
            let _ = tx.send(DownloadEvent::Error {
                item_id,
                msg: format!("No stream URL for track {}", track_num),
            });
            return;
        };

        let file_name = track_filename(track_num, &title);
        let file_path = dir.join(&file_name);

        if file_path.exists() {
            let _ = tx.send(DownloadEvent::TrackDone { item_id, track_num });
            return;
        }

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
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
                        return;
                    }
                    let _ = tx.send(DownloadEvent::TrackDone { item_id, track_num });
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
    });

    Ok(rx)
}

/// Prepare the library index entry for an album that's about to be downloaded.
pub fn prepare_album_entry(album: &Album) -> DownloadedAlbum {
    let tracks = album
        .tracks
        .iter()
        .map(|t| DownloadedTrack {
            track_num: t.track_num,
            title: t.title.clone(),
            file_name: track_filename(t.track_num, &t.title),
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
