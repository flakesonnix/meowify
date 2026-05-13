use serde::{Deserialize, Serialize};

use crate::rbac::{PartyPermission, PartyRole};

pub type RoomId = String;
pub type ClientId = String;
pub type QueueItemId = String;
pub type RequestId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomVisibility {
    Private,
    LanVisible,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomState {
    Creating,
    WaitingForClients,
    Active,
    Locked,
    PausedByAdminDisconnect,
    Ending,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Pending,
    Connected,
    Disconnected,
    Kicked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueItemStatus {
    Pending,
    Approved,
    Playing,
    Played,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackRef {
    YouTube {
        video_id: String,
        title: Option<String>,
        channel_title: Option<String>,
    },
    ImportedLocalFile {
        local_id: String,
        title: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSettings {
    pub lan_discovery: bool,
    pub bluetooth_experimental: bool,
    pub voting_enabled: bool,
    pub default_role: PartyRole,
}

impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            lan_discovery: false,
            bluetooth_experimental: false,
            voting_enabled: true,
            default_role: PartyRole::Client,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyRoom {
    pub room_id: RoomId,
    pub room_name: String,
    pub visibility: RoomVisibility,
    pub admin_client_id: ClientId,
    pub created_at_ms: u64,
    pub invite_code_hash: String,
    pub settings: RoomSettings,
    pub state: RoomState,
}

impl PartyRoom {
    pub fn accepts_join_requests(&self) -> bool {
        matches!(self.state, RoomState::WaitingForClients | RoomState::Active)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyClient {
    pub client_id: ClientId,
    pub device_name: String,
    pub user_name: String,
    pub role: PartyRole,
    pub permissions_override: Vec<PartyPermission>,
    pub connected_at_ms: u64,
    pub last_seen_ms: u64,
    pub connection_state: ConnectionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyQueueItem {
    pub queue_item_id: QueueItemId,
    pub track_ref: TrackRef,
    pub suggested_by: ClientId,
    pub votes: i32,
    pub status: QueueItemStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackSyncState {
    pub track_ref: Option<TrackRef>,
    pub position_ms: u64,
    pub is_playing: bool,
    pub updated_at_admin_clock_ms: u64,
    pub sequence_number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinRequest {
    pub request_id: RequestId,
    pub room_id: RoomId,
    pub client_id: ClientId,
    pub user_name: String,
    pub device_name: String,
    pub invite_code_attempt: Option<String>,
    pub requested_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSnapshot {
    pub room: PartyRoom,
    pub current_admin: ClientId,
    pub members: Vec<PartyClient>,
    pub pending_requests: Vec<JoinRequest>,
    pub queue: Vec<PartyQueueItem>,
    pub playback_state: PlaybackSyncState,
    pub protocol_version: u32,
}
