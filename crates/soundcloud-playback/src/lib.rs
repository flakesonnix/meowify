use meowify_core::can_persist_youtube_audio;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod gst;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    Off,
    One,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackStatus {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackSource {
    YouTubeVideo { video_id: String },
    SoundCloudTrack { track_id: String },
    ImportedLocalFile { path: String },
}

impl PlaybackSource {
    pub fn is_available_offline(&self) -> bool {
        match self {
            Self::YouTubeVideo { .. } => can_persist_youtube_audio(),
            Self::SoundCloudTrack { .. } => false,
            Self::ImportedLocalFile { .. } => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub source: PlaybackSource,
    pub title: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackQueue {
    items: Vec<QueueItem>,
    cursor: Option<usize>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueError {
    #[error("queue item not found")]
    ItemNotFound,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlaybackError {
    #[error("queue is empty")]
    QueueEmpty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackState {
    pub queue: PlaybackQueue,
    pub status: PlaybackStatus,
    pub repeat: RepeatMode,
    pub position_ms: u64,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            queue: PlaybackQueue::default(),
            status: PlaybackStatus::Stopped,
            repeat: RepeatMode::Off,
            position_ms: 0,
        }
    }
}

impl PlaybackQueue {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn cursor_index(&self) -> Option<usize> {
        self.cursor
    }

    pub fn push(&mut self, item: QueueItem) {
        self.items.push(item);
        if self.cursor.is_none() {
            self.cursor = Some(0);
        }
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.cursor.and_then(|index| self.items.get(index))
    }

    pub fn next(&mut self, repeat: RepeatMode) -> Option<&QueueItem> {
        let cursor = self.cursor?;

        self.cursor = match repeat {
            RepeatMode::One => Some(cursor),
            RepeatMode::Off if cursor + 1 >= self.items.len() => None,
            RepeatMode::Off => Some(cursor + 1),
            RepeatMode::All if self.items.is_empty() => None,
            RepeatMode::All => Some((cursor + 1) % self.items.len()),
        };

        self.current()
    }

    pub fn previous(&mut self, repeat: RepeatMode) -> Option<&QueueItem> {
        let cursor = self.cursor?;

        self.cursor = match repeat {
            RepeatMode::One => Some(cursor),
            RepeatMode::Off if cursor == 0 => None,
            RepeatMode::Off => Some(cursor - 1),
            RepeatMode::All if self.items.is_empty() => None,
            RepeatMode::All if cursor == 0 => Some(self.items.len() - 1),
            RepeatMode::All => Some(cursor - 1),
        };

        self.current()
    }

    pub fn select_first(&mut self) -> Option<&QueueItem> {
        if self.items.is_empty() {
            self.cursor = None;
        } else {
            self.cursor = Some(0);
        }

        self.current()
    }

    pub fn remove(&mut self, id: &str) -> Result<QueueItem, QueueError> {
        let index = self
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or(QueueError::ItemNotFound)?;
        let removed = self.items.remove(index);

        if self.items.is_empty() {
            self.cursor = None;
        } else if let Some(cursor) = self.cursor {
            self.cursor = Some(if index < cursor {
                cursor - 1
            } else {
                cursor.min(self.items.len() - 1)
            });
        }

        Ok(removed)
    }
}

impl PlaybackState {
    pub fn current(&self) -> Option<&QueueItem> {
        self.queue.current()
    }

    pub fn play(&mut self) -> Result<(), PlaybackError> {
        if self.queue.current().is_none() {
            self.queue.select_first().ok_or(PlaybackError::QueueEmpty)?;
        }

        self.status = PlaybackStatus::Playing;
        Ok(())
    }

    pub fn pause(&mut self) {
        if self.status == PlaybackStatus::Playing {
            self.status = PlaybackStatus::Paused;
        }
    }

    pub fn stop(&mut self) {
        self.status = PlaybackStatus::Stopped;
        self.position_ms = 0;
    }

    pub fn seek(&mut self, position_ms: u64) -> Result<(), PlaybackError> {
        if self.queue.current().is_none() {
            return Err(PlaybackError::QueueEmpty);
        }

        self.position_ms = position_ms;
        Ok(())
    }

    pub fn skip_next(&mut self) -> Option<&QueueItem> {
        self.position_ms = 0;
        let item = self.queue.next(self.repeat);
        if item.is_none() {
            self.status = PlaybackStatus::Stopped;
        }
        item
    }

    pub fn skip_previous(&mut self) -> Option<&QueueItem> {
        self.position_ms = 0;
        let item = self.queue.previous(self.repeat);
        if item.is_none() {
            self.status = PlaybackStatus::Stopped;
        }
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> QueueItem {
        QueueItem {
            id: id.to_string(),
            source: PlaybackSource::YouTubeVideo {
                video_id: format!("dQw4w9WgXcQ-{id}"),
            },
            title: id.to_string(),
        }
    }

    #[test]
    fn queue_advances_and_stops() {
        let mut queue = PlaybackQueue::default();
        queue.push(item("1"));
        queue.push(item("2"));

        assert_eq!(queue.current().unwrap().id, "1");
        assert_eq!(queue.next(RepeatMode::Off).unwrap().id, "2");
        assert!(queue.next(RepeatMode::Off).is_none());
    }

    #[test]
    fn repeat_all_wraps() {
        let mut queue = PlaybackQueue::default();
        queue.push(item("1"));
        queue.push(item("2"));

        assert_eq!(queue.next(RepeatMode::All).unwrap().id, "2");
        assert_eq!(queue.next(RepeatMode::All).unwrap().id, "1");
    }

    #[test]
    fn previous_repeat_all_wraps_to_tail() {
        let mut queue = PlaybackQueue::default();
        queue.push(item("1"));
        queue.push(item("2"));

        assert_eq!(queue.previous(RepeatMode::All).unwrap().id, "2");
    }

    #[test]
    fn removing_item_before_cursor_keeps_same_current_item() {
        let mut queue = PlaybackQueue::default();
        queue.push(item("1"));
        queue.push(item("2"));
        queue.push(item("3"));
        queue.next(RepeatMode::Off);
        queue.next(RepeatMode::Off);

        assert_eq!(queue.current().unwrap().id, "3");

        let removed = queue.remove("1").unwrap();

        assert_eq!(removed.id, "1");
        assert_eq!(queue.current().unwrap().id, "3");
        assert_eq!(queue.cursor_index(), Some(1));
    }

    #[test]
    fn removing_current_item_selects_next_item_at_same_index() {
        let mut queue = PlaybackQueue::default();
        queue.push(item("1"));
        queue.push(item("2"));
        queue.push(item("3"));
        queue.next(RepeatMode::Off);

        let removed = queue.remove("2").unwrap();

        assert_eq!(removed.id, "2");
        assert_eq!(queue.current().unwrap().id, "3");
        assert_eq!(queue.cursor_index(), Some(1));
    }

    #[test]
    fn playback_state_rejects_play_without_queue_item() {
        let mut playback = PlaybackState::default();

        assert_eq!(playback.play(), Err(PlaybackError::QueueEmpty));
        assert_eq!(playback.status, PlaybackStatus::Stopped);
    }

    #[test]
    fn playback_state_tracks_play_pause_seek_and_stop() {
        let mut playback = PlaybackState::default();
        playback.queue.push(item("1"));

        playback.play().unwrap();
        playback.seek(42_000).unwrap();
        playback.pause();

        assert_eq!(playback.status, PlaybackStatus::Paused);
        assert_eq!(playback.position_ms, 42_000);

        playback.stop();

        assert_eq!(playback.status, PlaybackStatus::Stopped);
        assert_eq!(playback.position_ms, 0);
    }

    #[test]
    fn playback_next_stops_at_end_without_repeat() {
        let mut playback = PlaybackState::default();
        playback.queue.push(item("1"));
        playback.play().unwrap();

        assert!(playback.skip_next().is_none());
        assert_eq!(playback.status, PlaybackStatus::Stopped);
        assert_eq!(playback.position_ms, 0);
    }

    #[test]
    fn youtube_sources_are_not_available_offline_by_default() {
        let youtube = PlaybackSource::YouTubeVideo {
            video_id: "dQw4w9WgXcQ".to_string(),
        };
        let imported = PlaybackSource::ImportedLocalFile {
            path: "/music/local.mp3".to_string(),
        };

        assert!(!youtube.is_available_offline());
        assert!(imported.is_available_offline());
    }
}
