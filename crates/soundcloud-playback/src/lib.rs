use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    Off,
    One,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackSource {
    SoundCloudTrack { urn: String },
    ImportedLocalFile { path: String },
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

impl PlaybackQueue {
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
            self.cursor = Some(cursor.min(self.items.len() - 1));
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> QueueItem {
        QueueItem {
            id: id.to_string(),
            source: PlaybackSource::SoundCloudTrack {
                urn: format!("soundcloud:tracks:{id}"),
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
}
