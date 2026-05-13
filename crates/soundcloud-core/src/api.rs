use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_track_access_from_api_json() {
        let parsed: TrackAccess = serde_json::from_str("\"playable\"").unwrap();
        assert_eq!(parsed, TrackAccess::Playable);
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
