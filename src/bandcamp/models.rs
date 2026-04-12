use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub item_id: u64,
    pub album_title: String,
    pub artist_name: String,
    pub item_url: String,
    pub art_url: Option<String>,
    pub date_added: Option<String>,
    pub tracks: Vec<Track>,
    pub about: Option<String>,
    pub credits: Option<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub sale_item_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub title: String,
    pub track_num: u32,
    pub duration: f64,
    pub stream_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthData {
    pub identity_cookie: String,
    pub fan_id: Option<u64>,
    pub username: Option<String>,
}

/// Intermediate structs for parsing Bandcamp API responses

#[derive(Debug, Deserialize)]
pub struct CollectionResponse {
    pub items: Vec<CollectionItem>,
    pub more_available: bool,
    pub last_token: Option<String>,
    #[serde(default)]
    pub redownload_urls: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct CollectionItem {
    pub sale_item_id: Option<u64>,
    pub item_id: Option<u64>,
    pub item_title: Option<String>,
    pub band_name: Option<String>,
    pub item_url: Option<String>,
    pub item_art_url: Option<String>,
    pub added: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TralbumData {
    pub trackinfo: Option<Vec<TrackInfo>>,
    pub current: Option<TralbumCurrent>,
}

#[derive(Debug, Deserialize)]
pub struct TralbumCurrent {
    pub about: Option<String>,
    pub credits: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TrackInfo {
    pub title: Option<String>,
    pub track_num: Option<u32>,
    pub duration: Option<f64>,
    pub file: Option<std::collections::HashMap<String, String>>,
}
