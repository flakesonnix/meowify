use std::collections::HashMap;

use thiserror::Error;

use crate::model::{
    ClientId, ConnectionState, JoinRequest, PartyClient, PartyQueueItem, PartyRoom,
    PlaybackSyncState, QueueItemId, QueueItemStatus, RequestId, RoomId, RoomSettings, RoomSnapshot,
    RoomState, RoomVisibility, TrackRef,
};
use crate::protocol::{JoinError, PlaybackCommandKind, hash_invite_code, validate_join_request};
use crate::rbac::{PartyPermission, PartyRole, PermissionError, require_permission};

pub const PROTOCOL_VERSION: u32 = crate::protocol::PROTOCOL_VERSION;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RoomError {
    #[error("join rejected: {0}")]
    JoinRejected(JoinError),
    #[error("client not found: {0}")]
    ClientNotFound(ClientId),
    #[error("{0}")]
    Permission(PermissionError),
    #[error("join request not found: {0}")]
    JoinRequestNotFound(RequestId),
    #[error("queue item not found: {0}")]
    QueueItemNotFound(QueueItemId),
    #[error("cannot target self")]
    CannotTargetSelf,
    #[error("cannot target admin")]
    CannotTargetAdmin,
}

impl From<JoinError> for RoomError {
    fn from(e: JoinError) -> Self {
        RoomError::JoinRejected(e)
    }
}

impl From<PermissionError> for RoomError {
    fn from(e: PermissionError) -> Self {
        RoomError::Permission(e)
    }
}

#[derive(Debug)]
pub struct RoomServer {
    room: PartyRoom,
    clients: HashMap<ClientId, PartyClient>,
    queue: Vec<PartyQueueItem>,
    playback: PlaybackSyncState,
    join_requests: HashMap<RequestId, JoinRequest>,
    sequence: u64,
}

impl RoomServer {
    pub fn create(
        room_id: impl Into<RoomId>,
        room_name: impl Into<String>,
        visibility: RoomVisibility,
        admin: PartyClient,
        invite_code: &str,
        now_ms: u64,
    ) -> Self {
        let room_id = room_id.into();
        let room = PartyRoom {
            invite_code_hash: hash_invite_code(invite_code, &room_id),
            admin_client_id: admin.client_id.clone(),
            room_id,
            room_name: room_name.into(),
            visibility,
            created_at_ms: now_ms,
            settings: RoomSettings::default(),
            state: RoomState::WaitingForClients,
        };

        let mut clients = HashMap::new();
        clients.insert(admin.client_id.clone(), admin);

        Self {
            room,
            clients,
            queue: Vec::new(),
            playback: PlaybackSyncState {
                track_ref: None,
                position_ms: 0,
                is_playing: false,
                updated_at_admin_clock_ms: now_ms,
                sequence_number: 0,
            },
            join_requests: HashMap::new(),
            sequence: 0,
        }
    }

    pub fn room(&self) -> &PartyRoom {
        &self.room
    }

    pub fn client(&self, client_id: &str) -> Option<&PartyClient> {
        self.clients.get(client_id)
    }

    pub fn queue(&self) -> &[PartyQueueItem] {
        &self.queue
    }

    pub fn playback(&self) -> &PlaybackSyncState {
        &self.playback
    }

    pub fn join_requests(&self) -> impl Iterator<Item = &JoinRequest> {
        self.join_requests.values()
    }

    pub fn snapshot(&self) -> RoomSnapshot {
        let mut members: Vec<PartyClient> = self.clients.values().cloned().collect();
        members.sort_by(|a, b| a.client_id.cmp(&b.client_id));
        let mut pending: Vec<JoinRequest> = self.join_requests.values().cloned().collect();
        pending.sort_by(|a, b| a.request_id.cmp(&b.request_id));
        RoomSnapshot {
            room: self.room.clone(),
            current_admin: self.room.admin_client_id.clone(),
            members,
            pending_requests: pending,
            queue: self.queue.clone(),
            playback_state: self.playback.clone(),
            protocol_version: PROTOCOL_VERSION,
        }
    }

    pub fn handle_join_request(&mut self, request: JoinRequest) -> Result<(), RoomError> {
        validate_join_request(&self.room, &request)?;
        self.join_requests
            .insert(request.request_id.clone(), request);
        Ok(())
    }

    pub fn approve_join(
        &mut self,
        approver_id: &str,
        request_id: &str,
        assigned_role: PartyRole,
        now_ms: u64,
    ) -> Result<PartyClient, RoomError> {
        let approver = self.require_client_owned(approver_id)?;
        require_permission(&approver, PartyPermission::ApproveJoin)?;

        let request = self
            .join_requests
            .remove(request_id)
            .ok_or_else(|| RoomError::JoinRequestNotFound(request_id.to_string()))?;

        let client = PartyClient {
            client_id: request.client_id.clone(),
            device_name: request.device_name,
            user_name: request.user_name,
            role: assigned_role,
            permissions_override: Vec::new(),
            connected_at_ms: now_ms,
            last_seen_ms: now_ms,
            connection_state: ConnectionState::Connected,
        };

        self.clients
            .insert(client.client_id.clone(), client.clone());

        if self.room.state == RoomState::WaitingForClients {
            self.room.state = RoomState::Active;
        }

        Ok(client)
    }

    pub fn reject_join(
        &mut self,
        rejecter_id: &str,
        request_id: &str,
    ) -> Result<JoinRequest, RoomError> {
        let rejecter = self.require_client_owned(rejecter_id)?;
        require_permission(&rejecter, PartyPermission::RejectJoin)?;

        self.join_requests
            .remove(request_id)
            .ok_or_else(|| RoomError::JoinRequestNotFound(request_id.to_string()))
    }

    pub fn remove_client(&mut self, client_id: &str) -> Result<(), RoomError> {
        self.clients
            .remove(client_id)
            .ok_or_else(|| RoomError::ClientNotFound(client_id.to_string()))?;
        Ok(())
    }

    pub fn kick_client(
        &mut self,
        issuer_id: &str,
        target_id: &str,
    ) -> Result<PartyClient, RoomError> {
        if issuer_id == target_id {
            return Err(RoomError::CannotTargetSelf);
        }
        if target_id == self.room.admin_client_id {
            return Err(RoomError::CannotTargetAdmin);
        }

        let issuer = self.require_client_owned(issuer_id)?;
        require_permission(&issuer, PartyPermission::KickClient)?;

        self.clients
            .remove(target_id)
            .ok_or_else(|| RoomError::ClientNotFound(target_id.to_string()))
    }

    pub fn set_role(
        &mut self,
        issuer_id: &str,
        target_id: &str,
        new_role: PartyRole,
    ) -> Result<PartyRole, RoomError> {
        if issuer_id == target_id {
            return Err(RoomError::CannotTargetSelf);
        }
        if target_id == self.room.admin_client_id {
            return Err(RoomError::CannotTargetAdmin);
        }

        let issuer = self.require_client_owned(issuer_id)?;
        let current_role = self
            .clients
            .get(target_id)
            .ok_or_else(|| RoomError::ClientNotFound(target_id.to_string()))?
            .role;

        // Lower discriminant = higher privilege (Admin < Moderator < Client < Guest).
        let permission = if new_role < current_role {
            PartyPermission::PromoteClient
        } else {
            PartyPermission::DemoteClient
        };
        require_permission(&issuer, permission)?;

        self.clients.get_mut(target_id).unwrap().role = new_role;
        Ok(new_role)
    }

    pub fn transfer_admin(&mut self, issuer_id: &str, target_id: &str) -> Result<(), RoomError> {
        if issuer_id == target_id {
            return Err(RoomError::CannotTargetSelf);
        }

        let issuer = self.require_client_owned(issuer_id)?;
        require_permission(&issuer, PartyPermission::TransferAdmin)?;

        if !self.clients.contains_key(target_id) {
            return Err(RoomError::ClientNotFound(target_id.to_string()));
        }

        self.clients.get_mut(issuer_id).unwrap().role = PartyRole::Moderator;
        self.clients.get_mut(target_id).unwrap().role = PartyRole::Admin;
        self.room.admin_client_id = target_id.to_string();

        Ok(())
    }

    pub fn lock_room(&mut self, issuer_id: &str) -> Result<(), RoomError> {
        let issuer = self.require_client_owned(issuer_id)?;
        require_permission(&issuer, PartyPermission::LockRoom)?;
        self.room.state = RoomState::Locked;
        Ok(())
    }

    pub fn unlock_room(&mut self, issuer_id: &str) -> Result<(), RoomError> {
        let issuer = self.require_client_owned(issuer_id)?;
        require_permission(&issuer, PartyPermission::LockRoom)?;
        self.room.state = RoomState::Active;
        Ok(())
    }

    pub fn end_room(&mut self, issuer_id: &str) -> Result<(), RoomError> {
        let issuer = self.require_client_owned(issuer_id)?;
        require_permission(&issuer, PartyPermission::EndRoom)?;
        self.room.state = RoomState::Ended;
        Ok(())
    }

    /// Called by the server when the admin client's connection is lost.
    /// Pauses the room so clients know playback should stop until the admin returns.
    pub fn pause_by_admin_disconnect(&mut self) {
        if matches!(self.room.state, RoomState::Active | RoomState::Locked) {
            self.room.state = RoomState::PausedByAdminDisconnect;
        }
    }

    /// Called when the admin client reconnects after a disconnect.
    /// Restores the room to Active state.
    pub fn resume_after_admin_reconnect(&mut self) {
        if self.room.state == RoomState::PausedByAdminDisconnect {
            self.room.state = RoomState::Active;
        }
    }

    pub fn apply_playback_command(
        &mut self,
        issuer_id: &str,
        command: PlaybackCommandKind,
        now_ms: u64,
    ) -> Result<&PlaybackSyncState, RoomError> {
        let issuer = self.require_client_owned(issuer_id)?;

        let permission = match &command {
            PlaybackCommandKind::Seek { .. } => PartyPermission::SeekPlayback,
            _ => PartyPermission::ControlPlayback,
        };
        require_permission(&issuer, permission)?;

        self.sequence += 1;
        match command {
            PlaybackCommandKind::Play => self.playback.is_playing = true,
            PlaybackCommandKind::Pause => self.playback.is_playing = false,
            PlaybackCommandKind::Seek { position_ms } => {
                self.playback.position_ms = position_ms;
            }
            PlaybackCommandKind::Next | PlaybackCommandKind::Previous => {
                self.playback.position_ms = 0;
            }
            PlaybackCommandKind::SetTrack { track_ref } => {
                self.playback.track_ref = Some(track_ref);
                self.playback.position_ms = 0;
                self.playback.is_playing = true;
            }
        }

        self.playback.sequence_number = self.sequence;
        self.playback.updated_at_admin_clock_ms = now_ms;

        Ok(&self.playback)
    }

    pub fn add_queue_item(
        &mut self,
        issuer_id: &str,
        queue_item_id: impl Into<QueueItemId>,
        track_ref: TrackRef,
    ) -> Result<&PartyQueueItem, RoomError> {
        let issuer = self.require_client_owned(issuer_id)?;
        require_permission(&issuer, PartyPermission::SuggestTrack)?;

        self.queue.push(PartyQueueItem {
            queue_item_id: queue_item_id.into(),
            track_ref,
            suggested_by: issuer_id.to_string(),
            votes: 0,
            status: QueueItemStatus::Pending,
        });

        Ok(self.queue.last().unwrap())
    }

    pub fn remove_queue_item(
        &mut self,
        issuer_id: &str,
        item_id: &str,
    ) -> Result<PartyQueueItem, RoomError> {
        let index = self
            .queue
            .iter()
            .position(|i| i.queue_item_id == item_id)
            .ok_or_else(|| RoomError::QueueItemNotFound(item_id.to_string()))?;

        let issuer = self.require_client_owned(issuer_id)?;
        let permission = if self.queue[index].suggested_by == issuer_id {
            PartyPermission::RemoveOwnSuggestion
        } else {
            PartyPermission::RemoveAnyTrack
        };
        require_permission(&issuer, permission)?;

        Ok(self.queue.remove(index))
    }

    pub fn vote_queue_item(
        &mut self,
        issuer_id: &str,
        item_id: &str,
        up: bool,
    ) -> Result<i32, RoomError> {
        let issuer = self.require_client_owned(issuer_id)?;
        require_permission(&issuer, PartyPermission::VoteTrack)?;

        let item = self
            .queue
            .iter_mut()
            .find(|i| i.queue_item_id == item_id)
            .ok_or_else(|| RoomError::QueueItemNotFound(item_id.to_string()))?;

        item.votes += if up { 1 } else { -1 };
        Ok(item.votes)
    }

    fn require_client_owned(&self, client_id: &str) -> Result<PartyClient, RoomError> {
        self.clients
            .get(client_id)
            .cloned()
            .ok_or_else(|| RoomError::ClientNotFound(client_id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admin_client() -> PartyClient {
        PartyClient {
            client_id: "admin-1".to_string(),
            device_name: "laptop".to_string(),
            user_name: "Admin User".to_string(),
            role: PartyRole::Admin,
            permissions_override: Vec::new(),
            connected_at_ms: 1000,
            last_seen_ms: 1000,
            connection_state: ConnectionState::Connected,
        }
    }

    fn make_server() -> RoomServer {
        RoomServer::create(
            "room-1",
            "Test Room",
            RoomVisibility::LanVisible,
            admin_client(),
            "secret-code",
            1000,
        )
    }

    fn join_request(client_id: &str) -> JoinRequest {
        JoinRequest {
            request_id: format!("req-{client_id}"),
            room_id: "room-1".to_string(),
            client_id: client_id.to_string(),
            user_name: client_id.to_string(),
            device_name: "phone".to_string(),
            invite_code_attempt: None,
            requested_at_ms: 2000,
        }
    }

    fn add_client(server: &mut RoomServer, client_id: &str) {
        server.handle_join_request(join_request(client_id)).unwrap();
        server
            .approve_join(
                "admin-1",
                &format!("req-{client_id}"),
                PartyRole::Client,
                2000,
            )
            .unwrap();
    }

    fn youtube_track() -> TrackRef {
        TrackRef::YouTube {
            video_id: "dQw4w9WgXcQ".to_string(),
            title: Some("Never Gonna Give You Up".to_string()),
            channel_title: Some("Rick Astley".to_string()),
        }
    }

    #[test]
    fn creates_room_with_admin() {
        let server = make_server();

        assert_eq!(server.room().admin_client_id, "admin-1");
        assert_eq!(server.room().state, RoomState::WaitingForClients);
        assert!(server.client("admin-1").is_some());
    }

    #[test]
    fn join_request_approved_adds_client_and_activates_room() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        assert_eq!(server.client("client-1").unwrap().role, PartyRole::Client);
        assert_eq!(server.room().state, RoomState::Active);
    }

    #[test]
    fn join_request_rejected_removes_pending_request() {
        let mut server = make_server();
        server
            .handle_join_request(join_request("client-1"))
            .unwrap();
        server.reject_join("admin-1", "req-client-1").unwrap();

        assert!(server.client("client-1").is_none());
    }

    #[test]
    fn non_admin_cannot_approve_join() {
        let mut server = make_server();
        add_client(&mut server, "client-1");
        server
            .handle_join_request(join_request("client-2"))
            .unwrap();

        let result = server.approve_join("client-1", "req-client-2", PartyRole::Client, 3000);
        assert!(result.is_err());
    }

    #[test]
    fn locked_room_rejects_new_join_requests() {
        let mut server = make_server();
        server.lock_room("admin-1").unwrap();

        let result = server.handle_join_request(join_request("client-1"));
        assert!(result.is_err());
    }

    #[test]
    fn admin_can_kick_client() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        server.kick_client("admin-1", "client-1").unwrap();

        assert!(server.client("client-1").is_none());
    }

    #[test]
    fn cannot_kick_self() {
        let mut server = make_server();

        assert_eq!(
            server.kick_client("admin-1", "admin-1"),
            Err(RoomError::CannotTargetSelf)
        );
    }

    #[test]
    fn client_cannot_kick_others() {
        let mut server = make_server();
        add_client(&mut server, "client-1");
        add_client(&mut server, "client-2");

        assert!(server.kick_client("client-1", "client-2").is_err());
    }

    #[test]
    fn admin_can_promote_client_to_moderator() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        server
            .set_role("admin-1", "client-1", PartyRole::Moderator)
            .unwrap();

        assert_eq!(
            server.client("client-1").unwrap().role,
            PartyRole::Moderator
        );
    }

    #[test]
    fn transfer_admin_swaps_roles_and_updates_room_admin_id() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        server.transfer_admin("admin-1", "client-1").unwrap();

        assert_eq!(server.room().admin_client_id, "client-1");
        assert_eq!(server.client("client-1").unwrap().role, PartyRole::Admin);
        assert_eq!(server.client("admin-1").unwrap().role, PartyRole::Moderator);
    }

    #[test]
    fn admin_can_lock_and_unlock_room() {
        let mut server = make_server();

        server.lock_room("admin-1").unwrap();
        assert_eq!(server.room().state, RoomState::Locked);

        server.unlock_room("admin-1").unwrap();
        assert_eq!(server.room().state, RoomState::Active);
    }

    #[test]
    fn admin_can_end_room() {
        let mut server = make_server();

        server.end_room("admin-1").unwrap();
        assert_eq!(server.room().state, RoomState::Ended);
    }

    #[test]
    fn admin_disconnect_pauses_active_room() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        assert_eq!(server.room().state, RoomState::Active);
        server.pause_by_admin_disconnect();
        assert_eq!(server.room().state, RoomState::PausedByAdminDisconnect);
    }

    #[test]
    fn admin_reconnect_restores_active_state() {
        let mut server = make_server();
        add_client(&mut server, "client-1");
        server.pause_by_admin_disconnect();

        server.resume_after_admin_reconnect();
        assert_eq!(server.room().state, RoomState::Active);
    }

    #[test]
    fn pause_is_no_op_when_room_already_ended() {
        let mut server = make_server();
        server.end_room("admin-1").unwrap();

        server.pause_by_admin_disconnect();

        assert_eq!(server.room().state, RoomState::Ended);
    }

    #[test]
    fn resume_is_no_op_when_room_was_not_paused() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        server.resume_after_admin_reconnect();

        assert_eq!(server.room().state, RoomState::Active);
    }

    #[test]
    fn locked_room_also_pauses_on_admin_disconnect() {
        let mut server = make_server();
        add_client(&mut server, "client-1");
        server.lock_room("admin-1").unwrap();

        server.pause_by_admin_disconnect();
        assert_eq!(server.room().state, RoomState::PausedByAdminDisconnect);
    }

    #[test]
    fn admin_can_play_pause_seek() {
        let mut server = make_server();

        let state = server
            .apply_playback_command("admin-1", PlaybackCommandKind::Play, 5000)
            .unwrap();
        assert!(state.is_playing);

        let state = server
            .apply_playback_command(
                "admin-1",
                PlaybackCommandKind::Seek {
                    position_ms: 30_000,
                },
                5001,
            )
            .unwrap();
        assert_eq!(state.position_ms, 30_000);

        let state = server
            .apply_playback_command("admin-1", PlaybackCommandKind::Pause, 5002)
            .unwrap();
        assert!(!state.is_playing);
    }

    #[test]
    fn set_track_starts_playback_at_zero() {
        let mut server = make_server();

        let state = server
            .apply_playback_command(
                "admin-1",
                PlaybackCommandKind::SetTrack {
                    track_ref: youtube_track(),
                },
                6000,
            )
            .unwrap();

        assert!(state.is_playing);
        assert_eq!(state.position_ms, 0);
        assert!(state.track_ref.is_some());
    }

    #[test]
    fn client_cannot_control_playback() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        let result = server.apply_playback_command("client-1", PlaybackCommandKind::Play, 5000);
        assert!(result.is_err());
    }

    #[test]
    fn sequence_number_increments_on_each_command() {
        let mut server = make_server();

        server
            .apply_playback_command("admin-1", PlaybackCommandKind::Play, 1000)
            .unwrap();
        assert_eq!(server.playback().sequence_number, 1);

        server
            .apply_playback_command("admin-1", PlaybackCommandKind::Pause, 2000)
            .unwrap();
        assert_eq!(server.playback().sequence_number, 2);
    }

    #[test]
    fn client_can_suggest_and_remove_own_track() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        server
            .add_queue_item("client-1", "item-1", youtube_track())
            .unwrap();
        assert_eq!(server.queue().len(), 1);

        server.remove_queue_item("client-1", "item-1").unwrap();
        assert_eq!(server.queue().len(), 0);
    }

    #[test]
    fn client_cannot_remove_others_suggestion() {
        let mut server = make_server();
        add_client(&mut server, "client-1");
        add_client(&mut server, "client-2");

        server
            .add_queue_item("client-1", "item-1", youtube_track())
            .unwrap();

        assert!(server.remove_queue_item("client-2", "item-1").is_err());
    }

    #[test]
    fn admin_can_remove_any_track() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        server
            .add_queue_item("client-1", "item-1", youtube_track())
            .unwrap();
        server.remove_queue_item("admin-1", "item-1").unwrap();

        assert_eq!(server.queue().len(), 0);
    }

    #[test]
    fn client_can_vote_on_queue_item() {
        let mut server = make_server();
        add_client(&mut server, "client-1");

        server
            .add_queue_item("admin-1", "item-1", youtube_track())
            .unwrap();

        assert_eq!(
            server.vote_queue_item("client-1", "item-1", true).unwrap(),
            1
        );
        assert_eq!(
            server.vote_queue_item("client-1", "item-1", false).unwrap(),
            0
        );
    }

    #[test]
    fn snapshot_contains_all_room_state() {
        let mut server = make_server();
        add_client(&mut server, "client-1");
        server
            .add_queue_item("client-1", "item-1", youtube_track())
            .unwrap();

        let snapshot = server.snapshot();

        assert_eq!(snapshot.room.room_id, "room-1");
        assert_eq!(snapshot.members.len(), 2);
        assert_eq!(snapshot.queue.len(), 1);
        assert_eq!(snapshot.pending_requests.len(), 0);
        assert_eq!(snapshot.protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn snapshot_includes_pending_join_requests() {
        let mut server = make_server();
        server
            .handle_join_request(join_request("client-1"))
            .unwrap();
        server
            .handle_join_request(join_request("client-2"))
            .unwrap();

        let snap = server.snapshot();

        assert_eq!(snap.pending_requests.len(), 2);
        let client_ids: Vec<&str> = snap
            .pending_requests
            .iter()
            .map(|r| r.client_id.as_str())
            .collect();
        assert!(client_ids.contains(&"client-1"));
        assert!(client_ids.contains(&"client-2"));
    }

    #[test]
    fn snapshot_members_are_sorted_by_client_id() {
        let mut server = make_server();
        add_client(&mut server, "zzz-client");
        add_client(&mut server, "aaa-client");

        let snap = server.snapshot();
        let ids: Vec<&str> = snap.members.iter().map(|m| m.client_id.as_str()).collect();

        assert_eq!(ids, vec!["aaa-client", "admin-1", "zzz-client"]);
    }

    #[test]
    fn approved_request_is_removed_from_pending() {
        let mut server = make_server();
        server
            .handle_join_request(join_request("client-1"))
            .unwrap();

        assert_eq!(server.snapshot().pending_requests.len(), 1);

        server
            .approve_join("admin-1", "req-client-1", PartyRole::Client, 2000)
            .unwrap();

        assert_eq!(server.snapshot().pending_requests.len(), 0);
        assert_eq!(server.snapshot().members.len(), 2);
    }
}
