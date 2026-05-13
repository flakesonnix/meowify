use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error")]
    Sqlite(#[from] rusqlite::Error),
    #[error("system clock is before Unix epoch")]
    Clock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPlaylist {
    pub id: String,
    pub name: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPlaylistItem {
    pub id: String,
    pub playlist_id: String,
    pub position: i64,
    pub source_type: String,
    pub source_ref: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalFavoriteKind {
    Track,
    Playlist,
}

impl LocalFavoriteKind {
    fn as_str(self) -> &'static str {
        match self {
            LocalFavoriteKind::Track => "track",
            LocalFavoriteKind::Playlist => "playlist",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalFavorite {
    pub kind: LocalFavoriteKind,
    pub urn: String,
    pub title: String,
    pub tags: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalFollow {
    pub user_urn: String,
    pub username: String,
    pub tags: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedLocalFile {
    pub local_id: String,
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at_ms: i64,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let db = Self {
            conn: Connection::open(path)?,
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let db = Self {
            conn: Connection::open_in_memory()?,
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn migrate(&self) -> Result<(), DbError> {
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS local_playlists (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS local_playlist_items (
                id TEXT PRIMARY KEY,
                playlist_id TEXT NOT NULL REFERENCES local_playlists(id) ON DELETE CASCADE,
                position INTEGER NOT NULL,
                source_type TEXT NOT NULL,
                source_ref TEXT NOT NULL,
                title TEXT NOT NULL,
                added_at_ms INTEGER NOT NULL,
                UNIQUE(playlist_id, position)
            );

            CREATE TABLE IF NOT EXISTS local_follows (
                user_urn TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                tags TEXT,
                notes TEXT,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS local_favorites (
                kind TEXT NOT NULL,
                urn TEXT NOT NULL,
                title TEXT NOT NULL,
                tags TEXT,
                notes TEXT,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                PRIMARY KEY(kind, urn)
            );

            CREATE TABLE IF NOT EXISTS imported_files (
                local_id TEXT PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                artist TEXT,
                duration_ms INTEGER,
                created_at_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS cache_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );

            PRAGMA user_version = 1;
            "#,
        )?;
        Ok(())
    }

    pub fn create_playlist(&self, id: &str, name: &str) -> Result<LocalPlaylist, DbError> {
        let now = now_ms()?;
        self.conn.execute(
            "INSERT INTO local_playlists (id, name, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, now, now],
        )?;

        Ok(LocalPlaylist {
            id: id.to_string(),
            name: name.to_string(),
            created_at_ms: now,
            updated_at_ms: now,
        })
    }

    pub fn list_playlists(&self) -> Result<Vec<LocalPlaylist>, DbError> {
        let mut statement = self.conn.prepare(
            "SELECT id, name, created_at_ms, updated_at_ms FROM local_playlists ORDER BY name",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(LocalPlaylist {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at_ms: row.get(2)?,
                updated_at_ms: row.get(3)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn add_playlist_item(
        &self,
        id: &str,
        playlist_id: &str,
        source_type: &str,
        source_ref: &str,
        title: &str,
    ) -> Result<LocalPlaylistItem, DbError> {
        let position: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM local_playlist_items WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?;
        let now = now_ms()?;
        self.conn.execute(
            "INSERT INTO local_playlist_items (id, playlist_id, position, source_type, source_ref, title, added_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, playlist_id, position, source_type, source_ref, title, now],
        )?;

        Ok(LocalPlaylistItem {
            id: id.to_string(),
            playlist_id: playlist_id.to_string(),
            position,
            source_type: source_type.to_string(),
            source_ref: source_ref.to_string(),
            title: title.to_string(),
        })
    }

    pub fn playlist_items(&self, playlist_id: &str) -> Result<Vec<LocalPlaylistItem>, DbError> {
        let mut statement = self.conn.prepare(
            "SELECT id, playlist_id, position, source_type, source_ref, title FROM local_playlist_items WHERE playlist_id = ?1 ORDER BY position",
        )?;
        let rows = statement.query_map(params![playlist_id], |row| {
            Ok(LocalPlaylistItem {
                id: row.get(0)?,
                playlist_id: row.get(1)?,
                position: row.get(2)?,
                source_type: row.get(3)?,
                source_ref: row.get(4)?,
                title: row.get(5)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn upsert_follow(
        &self,
        user_urn: &str,
        username: &str,
        tags: Option<&str>,
        notes: Option<&str>,
    ) -> Result<(), DbError> {
        let now = now_ms()?;
        self.conn.execute(
            r#"
            INSERT INTO local_follows (user_urn, username, tags, notes, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?5)
            ON CONFLICT(user_urn) DO UPDATE SET
                username = excluded.username,
                tags = excluded.tags,
                notes = excluded.notes,
                updated_at_ms = excluded.updated_at_ms
            "#,
            params![user_urn, username, tags, notes, now],
        )?;
        Ok(())
    }

    pub fn follow(&self, user_urn: &str) -> Result<Option<LocalFollow>, DbError> {
        self.conn
            .query_row(
                "SELECT user_urn, username, tags, notes FROM local_follows WHERE user_urn = ?1",
                params![user_urn],
                |row| {
                    Ok(LocalFollow {
                        user_urn: row.get(0)?,
                        username: row.get(1)?,
                        tags: row.get(2)?,
                        notes: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn upsert_favorite(&self, favorite: &LocalFavorite) -> Result<(), DbError> {
        let now = now_ms()?;
        self.conn.execute(
            r#"
            INSERT INTO local_favorites (kind, urn, title, tags, notes, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
            ON CONFLICT(kind, urn) DO UPDATE SET
                title = excluded.title,
                tags = excluded.tags,
                notes = excluded.notes,
                updated_at_ms = excluded.updated_at_ms
            "#,
            params![
                favorite.kind.as_str(),
                favorite.urn,
                favorite.title,
                favorite.tags,
                favorite.notes,
                now
            ],
        )?;
        Ok(())
    }

    pub fn favorites(&self, kind: LocalFavoriteKind) -> Result<Vec<LocalFavorite>, DbError> {
        let mut statement = self.conn.prepare(
            "SELECT urn, title, tags, notes FROM local_favorites WHERE kind = ?1 ORDER BY title",
        )?;
        let rows = statement.query_map(params![kind.as_str()], |row| {
            Ok(LocalFavorite {
                kind,
                urn: row.get(0)?,
                title: row.get(1)?,
                tags: row.get(2)?,
                notes: row.get(3)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn add_imported_file(
        &self,
        local_id: &str,
        path: &str,
        title: &str,
        artist: Option<&str>,
        duration_ms: Option<i64>,
    ) -> Result<ImportedLocalFile, DbError> {
        let now = now_ms()?;
        self.conn.execute(
            "INSERT INTO imported_files (local_id, path, title, artist, duration_ms, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![local_id, path, title, artist, duration_ms, now],
        )?;

        Ok(ImportedLocalFile {
            local_id: local_id.to_string(),
            path: path.to_string(),
            title: title.to_string(),
            artist: artist.map(str::to_string),
            duration_ms,
            created_at_ms: now,
        })
    }

    pub fn imported_file(&self, local_id: &str) -> Result<Option<ImportedLocalFile>, DbError> {
        self.conn
            .query_row(
                "SELECT local_id, path, title, artist, duration_ms, created_at_ms FROM imported_files WHERE local_id = ?1",
                params![local_id],
                imported_file_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn imported_file_by_path(&self, path: &str) -> Result<Option<ImportedLocalFile>, DbError> {
        self.conn
            .query_row(
                "SELECT local_id, path, title, artist, duration_ms, created_at_ms FROM imported_files WHERE path = ?1",
                params![path],
                imported_file_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn list_imported_files(&self) -> Result<Vec<ImportedLocalFile>, DbError> {
        let mut statement = self.conn.prepare(
            "SELECT local_id, path, title, artist, duration_ms, created_at_ms FROM imported_files ORDER BY title, path",
        )?;
        let rows = statement.query_map([], imported_file_from_row)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }
}

fn imported_file_from_row(row: &rusqlite::Row<'_>) -> Result<ImportedLocalFile, rusqlite::Error> {
    Ok(ImportedLocalFile {
        local_id: row.get(0)?,
        path: row.get(1)?,
        title: row.get(2)?,
        artist: row.get(3)?,
        duration_ms: row.get(4)?,
        created_at_ms: row.get(5)?,
    })
}

fn now_ms() -> Result<i64, DbError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DbError::Clock)?;
    Ok(duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_database_schema() {
        let db = Database::open_in_memory().unwrap();
        let version: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, 1);
    }

    #[test]
    fn creates_playlist_and_adds_youtube_video_reference() {
        let db = Database::open_in_memory().unwrap();
        let playlist = db.create_playlist("playlist-1", "Ambient").unwrap();
        let item = db
            .add_playlist_item(
                "item-1",
                &playlist.id,
                "youtube",
                "youtube:video:dQw4w9WgXcQ",
                "Never Gonna Give You Up",
            )
            .unwrap();

        assert_eq!(item.position, 0);
        assert_eq!(db.list_playlists().unwrap()[0].name, "Ambient");
        assert_eq!(
            db.playlist_items(&playlist.id).unwrap()[0].source_ref,
            "youtube:video:dQw4w9WgXcQ"
        );
    }

    #[test]
    fn stores_local_follows() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_follow(
            "youtube:channel:UC_x5XG1OV2P6uZZ5FSM9Ttw",
            "artist",
            Some("ambient"),
            Some("local note"),
        )
        .unwrap();

        let follow = db
            .follow("youtube:channel:UC_x5XG1OV2P6uZZ5FSM9Ttw")
            .unwrap()
            .unwrap();
        assert_eq!(follow.username, "artist");
        assert_eq!(follow.tags.as_deref(), Some("ambient"));
    }

    #[test]
    fn stores_local_favorites() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_favorite(&LocalFavorite {
            kind: LocalFavoriteKind::Track,
            urn: "youtube:video:dQw4w9WgXcQ".to_string(),
            title: "Never Gonna Give You Up".to_string(),
            tags: Some("focus".to_string()),
            notes: None,
        })
        .unwrap();

        let favorites = db.favorites(LocalFavoriteKind::Track).unwrap();
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].urn, "youtube:video:dQw4w9WgXcQ");
    }

    #[test]
    fn stores_imported_local_files_for_offline_playback() {
        let db = Database::open_in_memory().unwrap();
        let imported = db
            .add_imported_file(
                "local-1",
                "/music/local-track.flac",
                "Local Track",
                Some("Local Artist"),
                Some(181_000),
            )
            .unwrap();

        assert_eq!(imported.local_id, "local-1");
        assert!(imported.created_at_ms > 0);

        let by_id = db.imported_file("local-1").unwrap().unwrap();
        let by_path = db
            .imported_file_by_path("/music/local-track.flac")
            .unwrap()
            .unwrap();
        let files = db.list_imported_files().unwrap();

        assert_eq!(by_id.title, "Local Track");
        assert_eq!(by_path.artist.as_deref(), Some("Local Artist"));
        assert_eq!(files, vec![imported]);
    }
}
