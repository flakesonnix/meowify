use std::fmt::Write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::model::{
    ClientId, JoinRequest, PartyClient, PartyQueueItem, PartyRoom, PlaybackSyncState, RoomId,
    RoomSnapshot, RoomVisibility, TrackRef,
};
use crate::rbac::{PartyPermission, PartyRole};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateEnvelope<T> {
    pub room_id: RoomId,
    pub client_id: ClientId,
    pub session_token: String,
    pub sequence_number: u64,
    pub timestamp_ms: u64,
    pub payload: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vote {
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackCommandKind {
    Play,
    Pause,
    Seek { position_ms: u64 },
    Next,
    Previous,
    SetTrack { track_ref: TrackRef },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum PartyMessage {
    RoomAnnounce {
        room_id: RoomId,
        room_name: String,
        visibility: RoomVisibility,
        admin_name: String,
        protocol_version: u32,
    },
    RoomCreate {
        room_name: String,
        visibility: RoomVisibility,
    },
    RoomCreated {
        room_id: RoomId,
        invite_code: String,
        admin_client_id: ClientId,
    },
    RoomJoinRequest {
        room_id: RoomId,
        invite_code: Option<String>,
        user_name: String,
        device_name: String,
        client_public_key: String,
    },
    RoomJoinApproved {
        room_id: RoomId,
        client_id: ClientId,
        assigned_role: PartyRole,
        session_token: String,
        server_time_ms: u64,
    },
    RoomJoinRejected {
        room_id: RoomId,
        reason: String,
    },
    RoomLeave {
        room_id: RoomId,
        client_id: ClientId,
    },
    RoomEnded {
        room_id: RoomId,
        reason: String,
    },
    RoomLocked {
        room_id: RoomId,
        locked: bool,
    },
    RoomMemberSnapshot {
        room_id: RoomId,
        members: Vec<PartyClient>,
    },
    RoomRoleChanged {
        room_id: RoomId,
        client_id: ClientId,
        new_role: PartyRole,
    },
    RoomPermissionChanged {
        room_id: RoomId,
        role: PartyRole,
        permissions: Vec<PartyPermission>,
    },
    AdminTransferRequest {
        room_id: RoomId,
        target_client_id: ClientId,
    },
    AdminTransferAccepted {
        room_id: RoomId,
        old_admin_id: ClientId,
        new_admin_id: ClientId,
    },
    PlaybackCommand {
        room_id: RoomId,
        command: PlaybackCommandKind,
        issued_by: ClientId,
        sequence_number: u64,
    },
    PlaybackState(PlaybackSyncState),
    QueueSnapshot {
        room_id: RoomId,
        queue: Vec<PartyQueueItem>,
    },
    QueueAddRequest {
        room_id: RoomId,
        track_ref: TrackRef,
        issued_by: ClientId,
    },
    QueueAddApproved {
        room_id: RoomId,
        queue_item: PartyQueueItem,
    },
    QueueRemoveRequest {
        room_id: RoomId,
        queue_item_id: String,
        issued_by: ClientId,
    },
    QueueVote {
        room_id: RoomId,
        queue_item_id: String,
        vote: Vote,
        issued_by: ClientId,
    },
    SkipVote {
        room_id: RoomId,
        issued_by: ClientId,
    },
    KickClient {
        room_id: RoomId,
        target_client_id: ClientId,
        reason: Option<String>,
    },
    RoomSnapshot(RoomSnapshot),
    Error {
        room_id: Option<RoomId>,
        code: String,
        message: String,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JoinError {
    #[error("room is locked or not accepting joins")]
    RoomClosed,
    #[error("invite code is invalid")]
    InvalidInviteCode,
}

pub fn hash_invite_code(code: &str, room_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(room_id.as_bytes());
    hasher.update(b":");
    hasher.update(code.as_bytes());
    let digest = hasher.finalize();

    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

pub fn verify_invite_code(room: &PartyRoom, code: &str) -> bool {
    hash_invite_code(code, &room.room_id) == room.invite_code_hash
}

pub fn validate_join_request(room: &PartyRoom, request: &JoinRequest) -> Result<(), JoinError> {
    if !room.accepts_join_requests() {
        return Err(JoinError::RoomClosed);
    }

    if room.visibility == RoomVisibility::Private {
        let code = request
            .invite_code_attempt
            .as_deref()
            .ok_or(JoinError::InvalidInviteCode)?;

        if !verify_invite_code(room, code) {
            return Err(JoinError::InvalidInviteCode);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RoomSettings, RoomState};

    fn room(state: RoomState, visibility: RoomVisibility) -> PartyRoom {
        PartyRoom {
            room_id: "room-1".to_string(),
            room_name: "Living room".to_string(),
            visibility,
            admin_client_id: "admin-1".to_string(),
            created_at_ms: 1,
            invite_code_hash: hash_invite_code("123456", "room-1"),
            settings: RoomSettings::default(),
            state,
        }
    }

    fn join_request(code: Option<&str>) -> JoinRequest {
        JoinRequest {
            request_id: "request-1".to_string(),
            room_id: "room-1".to_string(),
            client_id: "client-1".to_string(),
            user_name: "tester".to_string(),
            device_name: "laptop".to_string(),
            invite_code_attempt: code.map(str::to_string),
            requested_at_ms: 2,
        }
    }

    #[test]
    fn invite_code_hash_validates() {
        let room = room(RoomState::Active, RoomVisibility::Private);

        assert!(verify_invite_code(&room, "123456"));
        assert!(!verify_invite_code(&room, "000000"));
    }

    #[test]
    fn locked_room_rejects_joins() {
        let room = room(RoomState::Locked, RoomVisibility::LanVisible);

        assert_eq!(
            validate_join_request(&room, &join_request(None)),
            Err(JoinError::RoomClosed)
        );
    }

    #[test]
    fn private_room_requires_valid_invite_code() {
        let room = room(RoomState::Active, RoomVisibility::Private);

        assert_eq!(
            validate_join_request(&room, &join_request(Some("bad"))),
            Err(JoinError::InvalidInviteCode)
        );
        assert!(validate_join_request(&room, &join_request(Some("123456"))).is_ok());
    }

    #[test]
    fn protocol_serializes_with_type_tag() {
        let message = PartyMessage::RoomLocked {
            room_id: "room-1".to_string(),
            locked: true,
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("RoomLocked"));
    }
}
