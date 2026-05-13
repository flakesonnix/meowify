use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::PartyClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PartyRole {
    Admin,
    Moderator,
    Client,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PartyPermission {
    CreateRoom,
    DiscoverRooms,
    JoinRoom,
    ApproveJoin,
    RejectJoin,
    LockRoom,
    EndRoom,
    TransferAdmin,
    PromoteClient,
    DemoteClient,
    KickClient,
    ControlPlayback,
    SeekPlayback,
    ModifyQueue,
    SuggestTrack,
    RemoveOwnSuggestion,
    RemoveAnyTrack,
    VoteTrack,
    VoteSkip,
    ChangeRoomSettings,
    ViewMembers,
    SendChatMessage,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PermissionError {
    #[error("role {role:?} lacks permission {permission:?}")]
    Denied {
        role: PartyRole,
        permission: PartyPermission,
    },
}

pub fn can(role: PartyRole, permission: PartyPermission) -> bool {
    use PartyPermission::*;
    use PartyRole::*;

    match role {
        Admin => true,
        Moderator => matches!(
            permission,
            DiscoverRooms
                | JoinRoom
                | ControlPlayback
                | SeekPlayback
                | ModifyQueue
                | SuggestTrack
                | RemoveOwnSuggestion
                | RemoveAnyTrack
                | VoteTrack
                | VoteSkip
                | ViewMembers
                | SendChatMessage
        ),
        Client => matches!(
            permission,
            DiscoverRooms
                | JoinRoom
                | SuggestTrack
                | RemoveOwnSuggestion
                | VoteTrack
                | VoteSkip
                | ViewMembers
                | SendChatMessage
        ),
        Guest => matches!(permission, DiscoverRooms | JoinRoom | ViewMembers),
    }
}

pub fn require_permission(
    client: &PartyClient,
    permission: PartyPermission,
) -> Result<(), PermissionError> {
    if client.permissions_override.contains(&permission) || can(client.role, permission) {
        return Ok(());
    }

    Err(PermissionError::Denied {
        role: client.role,
        permission,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ConnectionState;

    fn client(role: PartyRole) -> PartyClient {
        PartyClient {
            client_id: "client-1".to_string(),
            device_name: "laptop".to_string(),
            user_name: "tester".to_string(),
            role,
            permissions_override: Vec::new(),
            connected_at_ms: 1,
            last_seen_ms: 1,
            connection_state: ConnectionState::Connected,
        }
    }

    #[test]
    fn admin_can_use_all_permissions() {
        for permission in [
            PartyPermission::CreateRoom,
            PartyPermission::ApproveJoin,
            PartyPermission::RejectJoin,
            PartyPermission::LockRoom,
            PartyPermission::EndRoom,
            PartyPermission::TransferAdmin,
            PartyPermission::PromoteClient,
            PartyPermission::DemoteClient,
            PartyPermission::KickClient,
            PartyPermission::ControlPlayback,
            PartyPermission::SeekPlayback,
            PartyPermission::ModifyQueue,
            PartyPermission::ChangeRoomSettings,
        ] {
            assert!(can(PartyRole::Admin, permission));
        }
    }

    #[test]
    fn normal_client_cannot_run_admin_commands() {
        let client = client(PartyRole::Client);

        for permission in [
            PartyPermission::ApproveJoin,
            PartyPermission::RejectJoin,
            PartyPermission::LockRoom,
            PartyPermission::EndRoom,
            PartyPermission::TransferAdmin,
            PartyPermission::PromoteClient,
            PartyPermission::KickClient,
            PartyPermission::ControlPlayback,
        ] {
            assert!(require_permission(&client, permission).is_err());
        }
    }

    #[test]
    fn guest_cannot_vote_by_default() {
        let guest = client(PartyRole::Guest);

        assert_eq!(
            require_permission(&guest, PartyPermission::VoteTrack),
            Err(PermissionError::Denied {
                role: PartyRole::Guest,
                permission: PartyPermission::VoteTrack,
            })
        );
    }

    #[test]
    fn override_grants_specific_permission() {
        let mut client = client(PartyRole::Client);
        client
            .permissions_override
            .push(PartyPermission::ControlPlayback);

        assert!(require_permission(&client, PartyPermission::ControlPlayback).is_ok());
        assert!(require_permission(&client, PartyPermission::EndRoom).is_err());
    }
}
