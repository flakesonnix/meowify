use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SOUNDCLOUD_API_BASE_URL: &str = "https://api.soundcloud.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackAccess {
    Playable,
    Preview,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundCloudTrack {
    pub urn: String,
    pub title: String,
    pub permalink_url: Option<String>,
    pub access: Option<TrackAccess>,
    pub streamable: bool,
    pub downloadable: bool,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Streams {
    pub hls_aac_160_url: Option<String>,
    pub hls_mp3_128_url: Option<String>,
    #[serde(default)]
    pub http_mp3_128_url: Option<String>,
    pub preview_mp3_128_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedCollection<T> {
    pub collection: Vec<T>,
    pub next_href: Option<String>,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("invalid API URL: {0}")]
    InvalidUrl(#[from] reqwest::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfflineStatus {
    StreamOnly,
    Downloadable,
    Cached,
    UnavailableOffline,
    ImportedLocalFile,
}

impl SoundCloudTrack {
    pub fn offline_status(&self) -> OfflineStatus {
        match self.access {
            Some(TrackAccess::Blocked) => OfflineStatus::UnavailableOffline,
            _ if self.downloadable && self.download_url.is_some() => OfflineStatus::Downloadable,
            _ => OfflineStatus::StreamOnly,
        }
    }
}

pub fn can_persist_soundcloud_audio() -> bool {
    false
}

#[derive(Debug, Clone)]
pub struct SoundCloudApiClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl SoundCloudApiClient {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self::with_base_url(SOUNDCLOUD_API_BASE_URL, access_token)
    }

    pub fn with_base_url(base_url: impl Into<String>, access_token: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
            access_token: access_token.into(),
        }
    }

    pub fn authorization_header_value(&self) -> String {
        format!("OAuth {}", self.access_token)
    }

    pub fn search_tracks_request(&self, query: &str) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/tracks")
            .query(&[("q", query), ("linked_partitioning", "true")])
            .build()
    }

    pub fn search_users_request(&self, query: &str) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/users")
            .query(&[("q", query), ("linked_partitioning", "true")])
            .build()
    }

    pub fn search_playlists_request(
        &self,
        query: &str,
    ) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/playlists")
            .query(&[("q", query), ("linked_partitioning", "true")])
            .build()
    }

    pub fn track_request(&self, track_urn: &str) -> Result<reqwest::Request, reqwest::Error> {
        self.get(&format!("/tracks/{track_urn}")).build()
    }

    pub fn streams_request(&self, track_urn: &str) -> Result<reqwest::Request, reqwest::Error> {
        self.get(&format!("/tracks/{track_urn}/streams")).build()
    }

    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.http
            .get(format!("{}{}", self.base_url, path))
            .header("Authorization", self.authorization_header_value())
            .header("Accept", "application/json; charset=utf-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_track_access_from_api_json() {
        let parsed: TrackAccess = serde_json::from_str("\"playable\"").unwrap();
        assert_eq!(parsed, TrackAccess::Playable);
    }

    #[test]
    fn parses_streams_response_shape() {
        let streams: Streams = serde_json::from_str(
            r#"{
              "hls_aac_160_url": "https://example.invalid/aac.m3u8",
              "hls_mp3_128_url": "https://example.invalid/mp3.m3u8",
              "preview_mp3_128_url": null
            }"#,
        )
        .unwrap();

        assert!(streams.hls_aac_160_url.unwrap().ends_with("aac.m3u8"));
        assert!(streams.preview_mp3_128_url.is_none());
    }

    #[test]
    fn builds_documented_track_search_request() {
        let client = SoundCloudApiClient::with_base_url("https://api.soundcloud.com", "token");
        let request = client.search_tracks_request("ambient").unwrap();

        assert_eq!(request.url().path(), "/tracks");
        assert!(request.url().query().unwrap().contains("q=ambient"));
        assert_eq!(
            request.headers().get("Authorization").unwrap(),
            "OAuth token"
        );
    }

    #[test]
    fn blocked_track_is_unavailable_offline() {
        let track = SoundCloudTrack {
            urn: "soundcloud:tracks:1".to_string(),
            title: "Blocked".to_string(),
            permalink_url: None,
            access: Some(TrackAccess::Blocked),
            streamable: false,
            downloadable: true,
            download_url: Some("https://api.soundcloud.com/tracks/1/download".to_string()),
        };

        assert_eq!(track.offline_status(), OfflineStatus::UnavailableOffline);
    }

    #[test]
    fn app_does_not_persist_soundcloud_audio_by_default() {
        assert!(!can_persist_soundcloud_audio());
    }
}
