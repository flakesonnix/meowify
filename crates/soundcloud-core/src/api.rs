use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const YOUTUBE_API_BASE_URL: &str = "https://www.googleapis.com/youtube/v3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoPrivacyStatus {
    Public,
    Unlisted,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YouTubeVideo {
    pub id: String,
    pub title: String,
    pub channel_id: String,
    pub channel_title: String,
    pub description: Option<String>,
    pub duration_iso: Option<String>,
    pub privacy_status: Option<VideoPrivacyStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YouTubeChannel {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub custom_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YouTubePlaylist {
    pub id: String,
    pub title: String,
    pub channel_id: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagedResponse<T> {
    pub items: Vec<T>,
    pub next_page_token: Option<String>,
    pub total_results: Option<u64>,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("invalid API URL: {0}")]
    InvalidUrl(#[from] reqwest::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfflineStatus {
    StreamOnly,
    UnavailableOffline,
    ImportedLocalFile,
}

impl YouTubeVideo {
    pub fn permalink_url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.id)
    }

    pub fn offline_status(&self) -> OfflineStatus {
        match self.privacy_status {
            Some(VideoPrivacyStatus::Private) => OfflineStatus::UnavailableOffline,
            _ => OfflineStatus::StreamOnly,
        }
    }
}

pub fn can_persist_youtube_audio() -> bool {
    false
}

#[derive(Debug, Clone)]
pub struct YouTubeApiClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl YouTubeApiClient {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self::with_base_url(YOUTUBE_API_BASE_URL, access_token)
    }

    pub fn with_base_url(base_url: impl Into<String>, access_token: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
            access_token: access_token.into(),
        }
    }

    pub fn authorization_header_value(&self) -> String {
        format!("Bearer {}", self.access_token)
    }

    pub fn search_videos_request(&self, query: &str) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/search")
            .query(&[
                ("part", "snippet"),
                ("type", "video"),
                ("q", query),
                ("maxResults", "50"),
            ])
            .build()
    }

    pub fn search_channels_request(&self, query: &str) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/search")
            .query(&[
                ("part", "snippet"),
                ("type", "channel"),
                ("q", query),
                ("maxResults", "50"),
            ])
            .build()
    }

    pub fn search_playlists_request(
        &self,
        query: &str,
    ) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/search")
            .query(&[
                ("part", "snippet"),
                ("type", "playlist"),
                ("q", query),
                ("maxResults", "50"),
            ])
            .build()
    }

    pub fn video_request(&self, video_id: &str) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/videos")
            .query(&[("part", "snippet,contentDetails,status"), ("id", video_id)])
            .build()
    }

    pub fn channel_request(&self, channel_id: &str) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/channels")
            .query(&[("part", "snippet"), ("id", channel_id)])
            .build()
    }

    pub fn playlist_request(&self, playlist_id: &str) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/playlists")
            .query(&[("part", "snippet"), ("id", playlist_id)])
            .build()
    }

    pub fn playlist_items_request(
        &self,
        playlist_id: &str,
    ) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/playlistItems")
            .query(&[
                ("part", "snippet,contentDetails"),
                ("playlistId", playlist_id),
                ("maxResults", "50"),
            ])
            .build()
    }

    pub fn channel_playlists_request(
        &self,
        channel_id: &str,
    ) -> Result<reqwest::Request, reqwest::Error> {
        self.get("/playlists")
            .query(&[
                ("part", "snippet"),
                ("channelId", channel_id),
                ("maxResults", "50"),
            ])
            .build()
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
    fn parses_video_privacy_status_from_api_json() {
        let parsed: VideoPrivacyStatus = serde_json::from_str("\"public\"").unwrap();
        assert_eq!(parsed, VideoPrivacyStatus::Public);
    }

    #[test]
    fn builds_documented_video_search_request() {
        let client =
            YouTubeApiClient::with_base_url("https://www.googleapis.com/youtube/v3", "token");
        let request = client.search_videos_request("ambient").unwrap();

        assert_eq!(request.url().path(), "/youtube/v3/search");
        assert!(request.url().query().unwrap().contains("q=ambient"));
        assert!(request.url().query().unwrap().contains("type=video"));
        assert_eq!(
            request.headers().get("Authorization").unwrap(),
            "Bearer token"
        );
    }

    #[test]
    fn private_video_is_unavailable_offline() {
        let video = YouTubeVideo {
            id: "dQw4w9WgXcQ".to_string(),
            title: "Private".to_string(),
            channel_id: "UC_x5XG1OV2P6uZZ5FSM9Ttw".to_string(),
            channel_title: "Example".to_string(),
            description: None,
            duration_iso: None,
            privacy_status: Some(VideoPrivacyStatus::Private),
        };

        assert_eq!(video.offline_status(), OfflineStatus::UnavailableOffline);
    }

    #[test]
    fn public_video_is_stream_only() {
        let video = YouTubeVideo {
            id: "dQw4w9WgXcQ".to_string(),
            title: "Public".to_string(),
            channel_id: "UC_x5XG1OV2P6uZZ5FSM9Ttw".to_string(),
            channel_title: "Example".to_string(),
            description: None,
            duration_iso: Some("PT3M33S".to_string()),
            privacy_status: Some(VideoPrivacyStatus::Public),
        };

        assert_eq!(video.offline_status(), OfflineStatus::StreamOnly);
    }

    #[test]
    fn app_does_not_persist_youtube_audio_by_default() {
        assert!(!can_persist_youtube_audio());
    }

    #[test]
    fn permalink_url_uses_standard_watch_format() {
        let video = YouTubeVideo {
            id: "dQw4w9WgXcQ".to_string(),
            title: "Example".to_string(),
            channel_id: "UC_x5XG1OV2P6uZZ5FSM9Ttw".to_string(),
            channel_title: "Example".to_string(),
            description: None,
            duration_iso: None,
            privacy_status: None,
        };

        assert_eq!(
            video.permalink_url(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
    }
}
