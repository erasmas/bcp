use anyhow::{Context, Result};
use reqwest::header::{COOKIE, USER_AGENT};

use super::models::*;

const BC_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct BandcampClient {
    http: reqwest::Client,
    identity_cookie: String,
}

impl BandcampClient {
    pub fn new(identity_cookie: String) -> Self {
        let http = reqwest::Client::builder()
            .build()
            .expect("Failed to build HTTP client");
        Self {
            http,
            identity_cookie,
        }
    }

    fn cookie_header(&self) -> String {
        format!("identity={}", self.identity_cookie)
    }

    /// Fetch the fan_id and username via the collection_summary API
    pub async fn fetch_fan_info(&self) -> Result<(u64, String)> {
        let resp = self
            .http
            .get("https://bandcamp.com/api/fan/2/collection_summary")
            .header(COOKIE, self.cookie_header())
            .header(USER_AGENT, BC_USER_AGENT)
            .send()
            .await
            .context("Failed to fetch collection summary")?;

        let status = resp.status();
        let body = resp.text().await?;

        if !status.is_success() {
            anyhow::bail!(
                "Collection summary API returned {} — cookie may be expired.\nCookie value: {}...",
                status,
                &self.identity_cookie[..self.identity_cookie.len().min(20)]
            );
        }

        let data: serde_json::Value =
            serde_json::from_str(&body).context("Failed to parse collection summary")?;

        let fan_id = data
            .get("fan_id")
            .and_then(|v| v.as_u64())
            .context("No fan_id in collection summary response")?;

        let username = data
            .pointer("/collection_summary/username")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok((fan_id, username))
    }

    /// Fetch the user's collection (purchased albums)
    pub async fn fetch_collection(
        &self,
        fan_id: u64,
        older_than_token: Option<&str>,
    ) -> Result<CollectionResponse> {
        let default_token = format!("{}::a::", chrono_like_timestamp());
        let token = older_than_token.unwrap_or(&default_token);

        let body = serde_json::json!({
            "fan_id": fan_id,
            "older_than_token": token,
            "count": 40,
        });

        let resp = self
            .http
            .post("https://bandcamp.com/api/fancollection/1/collection_items")
            .header(COOKIE, self.cookie_header())
            .header(USER_AGENT, BC_USER_AGENT)
            .json(&body)
            .send()
            .await
            .context("Failed to fetch collection")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Collection API returned {}: {}", status, text);
        }

        let data: CollectionResponse = resp
            .json()
            .await
            .context("Failed to parse collection response")?;
        Ok(data)
    }

    /// Fetch all collection pages
    pub async fn fetch_full_collection(&self, fan_id: u64) -> Result<Vec<Album>> {
        let mut albums = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();
        let mut token: Option<String> = None;

        loop {
            let resp = self.fetch_collection(fan_id, token.as_deref()).await?;

            for item in &resp.items {
                let id = item.sale_item_id.or(item.item_id).unwrap_or(0);
                if !seen_ids.insert(id) {
                    continue; // skip duplicate
                }
                let album = Album {
                    item_id: id,
                    album_title: item.item_title.clone().unwrap_or_default(),
                    artist_name: item.band_name.clone().unwrap_or_default(),
                    item_url: item.item_url.clone().unwrap_or_default(),
                    art_url: item.item_art_url.clone(),
                    date_added: item.added.clone(),
                    tracks: Vec::new(),
                    about: None,
                    credits: None,
                    release_date: None,
                };
                albums.push(album);
            }

            if !resp.more_available {
                break;
            }
            token = resp.last_token;
            if token.is_none() {
                break;
            }
        }

        Ok(albums)
    }

    /// Fetch track listing, stream URLs, and album metadata
    pub async fn fetch_album_details(&self, album_url: &str) -> Result<AlbumDetail> {
        let resp = self
            .http
            .get(album_url)
            .header(COOKIE, self.cookie_header())
            .header(USER_AGENT, BC_USER_AGENT)
            .send()
            .await
            .context("Failed to fetch album page")?;

        let body = resp.text().await?;
        parse_album_page(&body)
    }
}

pub struct AlbumDetail {
    pub tracks: Vec<Track>,
    pub about: Option<String>,
    pub credits: Option<String>,
    pub release_date: Option<String>,
}

fn parse_album_page(html: &str) -> Result<AlbumDetail> {
    let tralbum_json =
        extract_data_tralbum(html).context("Could not find track data on album page")?;

    let data: TralbumData =
        serde_json::from_str(&tralbum_json).context("Failed to parse tralbum data")?;

    let tracks = data
        .trackinfo
        .unwrap_or_default()
        .into_iter()
        .map(|t| {
            let stream_url = t.file.as_ref().and_then(|f| f.get("mp3-128").cloned());

            Track {
                title: t.title.unwrap_or_else(|| "Untitled".to_string()),
                track_num: t.track_num.unwrap_or(0),
                duration: t.duration.unwrap_or(0.0),
                stream_url,
            }
        })
        .collect();

    let (about, credits, release_date) = match data.current {
        Some(current) => (current.about, current.credits, current.release_date),
        None => (None, None, None),
    };

    Ok(AlbumDetail {
        tracks,
        about,
        credits,
        release_date,
    })
}

fn extract_data_tralbum(html: &str) -> Option<String> {
    // Pattern: data-tralbum="..." (HTML-encoded JSON)
    let marker = "data-tralbum=\"";
    let start = html.find(marker)? + marker.len();
    let rest = &html[start..];
    let end = rest.find('"')?;
    let encoded = &rest[..end];

    // Decode HTML entities
    let decoded = encoded
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'");

    Some(decoded)
}

fn chrono_like_timestamp() -> String {
    // Generate a Unix timestamp for "now" used as the initial token
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}
