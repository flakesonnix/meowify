use std::fmt::Write as _;

use clap::{Parser, Subcommand, ValueEnum};
use meowify_party::{
    ConnectionState, JoinRequest, PROTOCOL_VERSION, PartyClient, PartyPermission, PartyRole,
    PlaybackCommandKind, RoomServer, RoomState, RoomVisibility, TrackRef, can,
};

#[derive(Debug, Parser)]
#[command(name = "meowify-cli")]
#[command(about = "Meowify debugging/admin CLI")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Status,
    Party {
        #[command(subcommand)]
        command: PartyCommand,
    },
}

#[derive(Debug, Subcommand)]
enum PartyCommand {
    Create,
    Rooms,
    Join {
        room_id: String,
    },
    JoinCode {
        code: String,
    },
    Leave,
    Members,
    Approve {
        client_id: String,
    },
    Reject {
        client_id: String,
    },
    Promote {
        client_id: String,
    },
    Demote {
        client_id: String,
    },
    TransferAdmin {
        client_id: String,
    },
    Kick {
        client_id: String,
    },
    Lock,
    Unlock,
    End,
    Queue,
    Vote {
        queue_item_id: String,
    },
    Suggest {
        track_ref: String,
    },
    Permissions {
        #[arg(long, value_enum)]
        role: Option<RoleArg>,
    },
    Snapshot {
        #[arg(
            long,
            help = "Output machine-readable JSON instead of human-readable text"
        )]
        json: bool,
        #[arg(long, value_enum, help = "Only output if room is in this state")]
        filter_state: Option<RoomStateArg>,
        #[arg(long, help = "Only output if room has at least this many members")]
        filter_min_members: Option<usize>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RoleArg {
    Admin,
    Moderator,
    Client,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RoomStateArg {
    Waiting,
    Active,
    Locked,
    Paused,
    Ended,
}

impl RoomStateArg {
    fn matches(self, state: RoomState) -> bool {
        matches!(
            (self, state),
            (Self::Waiting, RoomState::WaitingForClients)
                | (Self::Active, RoomState::Active)
                | (Self::Locked, RoomState::Locked)
                | (Self::Paused, RoomState::PausedByAdminDisconnect)
                | (Self::Ended, RoomState::Ended)
        )
    }
}

fn main() {
    let args = Args::parse();
    print!("{}", render_command(args.command.as_ref()));
}

fn render_command(command: Option<&Command>) -> String {
    match command {
        None | Some(Command::Status) => render_status(),
        Some(Command::Party { command }) => render_party_command(command),
    }
}

fn render_status() -> String {
    format!(
        "Meowify CLI\nplatform: YouTube\nparty protocol: v{PROTOCOL_VERSION}\nnetwork: not started by debug CLI\n"
    )
}

fn render_party_command(command: &PartyCommand) -> String {
    match command {
        PartyCommand::Create => render_party_stub("create room", Some(PartyPermission::CreateRoom)),
        PartyCommand::Rooms => {
            render_party_stub("list rooms", Some(PartyPermission::DiscoverRooms))
        }
        PartyCommand::Join { room_id } => render_party_stub(
            &format!("join room {room_id}"),
            Some(PartyPermission::JoinRoom),
        ),
        PartyCommand::JoinCode { code } => render_party_stub(
            &format!("join with invite code {code}"),
            Some(PartyPermission::JoinRoom),
        ),
        PartyCommand::Leave => render_party_stub("leave room", None),
        PartyCommand::Members => {
            render_party_stub("list members", Some(PartyPermission::ViewMembers))
        }
        PartyCommand::Approve { client_id } => render_party_stub(
            &format!("approve client {client_id}"),
            Some(PartyPermission::ApproveJoin),
        ),
        PartyCommand::Reject { client_id } => render_party_stub(
            &format!("reject client {client_id}"),
            Some(PartyPermission::RejectJoin),
        ),
        PartyCommand::Promote { client_id } => render_party_stub(
            &format!("promote client {client_id}"),
            Some(PartyPermission::PromoteClient),
        ),
        PartyCommand::Demote { client_id } => render_party_stub(
            &format!("demote client {client_id}"),
            Some(PartyPermission::DemoteClient),
        ),
        PartyCommand::TransferAdmin { client_id } => render_party_stub(
            &format!("transfer admin to {client_id}"),
            Some(PartyPermission::TransferAdmin),
        ),
        PartyCommand::Kick { client_id } => render_party_stub(
            &format!("kick client {client_id}"),
            Some(PartyPermission::KickClient),
        ),
        PartyCommand::Lock => render_party_stub("lock room", Some(PartyPermission::LockRoom)),
        PartyCommand::Unlock => render_party_stub("unlock room", Some(PartyPermission::LockRoom)),
        PartyCommand::End => render_party_stub("end room", Some(PartyPermission::EndRoom)),
        PartyCommand::Queue => render_party_stub("show queue", Some(PartyPermission::ViewMembers)),
        PartyCommand::Vote { queue_item_id } => render_party_stub(
            &format!("vote for queue item {queue_item_id}"),
            Some(PartyPermission::VoteTrack),
        ),
        PartyCommand::Suggest { track_ref } => render_party_stub(
            &format!("suggest track ref {track_ref}"),
            Some(PartyPermission::SuggestTrack),
        ),
        PartyCommand::Permissions { role } => render_permissions(*role),
        PartyCommand::Snapshot {
            json,
            filter_state,
            filter_min_members,
        } => render_snapshot(*json, *filter_state, *filter_min_members),
    }
}

fn render_party_stub(action: &str, required: Option<PartyPermission>) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "party action: {action}");
    let _ = writeln!(
        output,
        "status: parsed; network handler not implemented yet"
    );
    if let Some(permission) = required {
        let _ = writeln!(output, "required permission: {permission:?}");
    }
    let _ = writeln!(output, "policy: enforce RBAC before state changes");
    output
}

fn render_permissions(role_filter: Option<RoleArg>) -> String {
    let roles: Vec<RoleArg> = role_filter.map(|role| vec![role]).unwrap_or_else(|| {
        vec![
            RoleArg::Admin,
            RoleArg::Moderator,
            RoleArg::Client,
            RoleArg::Guest,
        ]
    });
    let mut output = String::from("Party permissions\n");

    for role in roles {
        let allowed = ALL_PERMISSIONS
            .iter()
            .copied()
            .filter(|permission| can(role.party_role(), *permission))
            .map(permission_name)
            .collect::<Vec<_>>()
            .join(", ");

        let _ = writeln!(output, "{}: {allowed}", role.name());
    }

    output
}

fn render_snapshot(
    json: bool,
    filter_state: Option<RoomStateArg>,
    filter_min_members: Option<usize>,
) -> String {
    let admin = PartyClient {
        client_id: "admin-1".to_string(),
        device_name: "laptop".to_string(),
        user_name: "Alice".to_string(),
        role: PartyRole::Admin,
        permissions_override: Vec::new(),
        connected_at_ms: 0,
        last_seen_ms: 0,
        connection_state: ConnectionState::Connected,
    };

    let mut server = RoomServer::create(
        "room-demo",
        "Demo Room",
        RoomVisibility::LanVisible,
        admin,
        "demo-invite",
        0,
    );

    server
        .handle_join_request(JoinRequest {
            request_id: "req-bob".to_string(),
            room_id: "room-demo".to_string(),
            client_id: "client-bob".to_string(),
            user_name: "Bob".to_string(),
            device_name: "phone".to_string(),
            invite_code_attempt: None,
            requested_at_ms: 500,
        })
        .unwrap();
    server
        .approve_join("admin-1", "req-bob", PartyRole::Client, 1000)
        .unwrap();

    server
        .add_queue_item(
            "admin-1",
            "item-1",
            TrackRef::YouTube {
                video_id: "dQw4w9WgXcQ".to_string(),
                title: Some("Never Gonna Give You Up".to_string()),
                channel_title: Some("Rick Astley".to_string()),
            },
        )
        .unwrap();

    server
        .add_queue_item(
            "client-bob",
            "item-2",
            TrackRef::YouTube {
                video_id: "9bZkp7q19f0".to_string(),
                title: Some("GANGNAM STYLE".to_string()),
                channel_title: Some("officialpsy".to_string()),
            },
        )
        .unwrap();

    server
        .apply_playback_command(
            "admin-1",
            PlaybackCommandKind::SetTrack {
                track_ref: TrackRef::YouTube {
                    video_id: "dQw4w9WgXcQ".to_string(),
                    title: Some("Never Gonna Give You Up".to_string()),
                    channel_title: Some("Rick Astley".to_string()),
                },
            },
            2000,
        )
        .unwrap();

    let snap = server.snapshot();

    if let Some(state_filter) = filter_state {
        if !state_filter.matches(snap.room.state) {
            return "No rooms match the filter.\n".to_string();
        }
    }
    if let Some(min) = filter_min_members {
        if snap.members.len() < min {
            return "No rooms match the filter.\n".to_string();
        }
    }

    if json {
        serde_json::to_string_pretty(&snap).expect("RoomSnapshot serializes")
    } else {
        render_room_snapshot(&snap)
    }
}

fn render_room_snapshot(snap: &meowify_party::RoomSnapshot) -> String {
    let mut out = String::new();
    let room = &snap.room;

    let _ = writeln!(
        out,
        "=== Room Snapshot (protocol v{}) ===",
        snap.protocol_version
    );
    let _ = writeln!(out, "room_id:    {}", room.room_id);
    let _ = writeln!(out, "name:       {}", room.room_name);
    let _ = writeln!(out, "state:      {:?}", room.state);
    let _ = writeln!(out, "visibility: {:?}", room.visibility);
    let _ = writeln!(out, "admin:      {}", snap.current_admin);
    let _ = writeln!(out);

    let _ = writeln!(out, "--- Members ({}) ---", snap.members.len());
    let mut members = snap.members.clone();
    members.sort_by(|a, b| a.client_id.cmp(&b.client_id));
    for m in &members {
        let _ = writeln!(
            out,
            "  {} | {:?} | {:?} | {}",
            m.client_id, m.role, m.connection_state, m.user_name
        );
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "--- Queue ({} items) ---", snap.queue.len());
    for item in &snap.queue {
        let track_label = match &item.track_ref {
            TrackRef::YouTube {
                title,
                channel_title,
                ..
            } => format!(
                "{} — {}",
                title.as_deref().unwrap_or("(no title)"),
                channel_title.as_deref().unwrap_or("(no channel)")
            ),
            TrackRef::ImportedLocalFile { title, .. } => format!("[local] {title}"),
        };
        let _ = writeln!(
            out,
            "  {} | votes:{:+} | by:{} | {}",
            item.queue_item_id, item.votes, item.suggested_by, track_label
        );
    }
    let _ = writeln!(out);

    let pb = &snap.playback_state;
    let _ = writeln!(out, "--- Playback ---");
    let _ = writeln!(out, "  playing:  {}", pb.is_playing);
    let _ = writeln!(out, "  position: {} ms", pb.position_ms);
    let _ = writeln!(out, "  seq:      {}", pb.sequence_number);
    match &pb.track_ref {
        Some(TrackRef::YouTube {
            title, video_id, ..
        }) => {
            let _ = writeln!(
                out,
                "  track:    {} ({})",
                title.as_deref().unwrap_or("(no title)"),
                video_id
            );
        }
        Some(TrackRef::ImportedLocalFile { title, local_id }) => {
            let _ = writeln!(out, "  track:    [local] {title} ({local_id})");
        }
        None => {
            let _ = writeln!(out, "  track:    none");
        }
    }

    out
}

impl RoleArg {
    fn party_role(self) -> PartyRole {
        match self {
            Self::Admin => PartyRole::Admin,
            Self::Moderator => PartyRole::Moderator,
            Self::Client => PartyRole::Client,
            Self::Guest => PartyRole::Guest,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Moderator => "moderator",
            Self::Client => "client",
            Self::Guest => "guest",
        }
    }
}

fn permission_name(permission: PartyPermission) -> &'static str {
    match permission {
        PartyPermission::CreateRoom => "CreateRoom",
        PartyPermission::DiscoverRooms => "DiscoverRooms",
        PartyPermission::JoinRoom => "JoinRoom",
        PartyPermission::ApproveJoin => "ApproveJoin",
        PartyPermission::RejectJoin => "RejectJoin",
        PartyPermission::LockRoom => "LockRoom",
        PartyPermission::EndRoom => "EndRoom",
        PartyPermission::TransferAdmin => "TransferAdmin",
        PartyPermission::PromoteClient => "PromoteClient",
        PartyPermission::DemoteClient => "DemoteClient",
        PartyPermission::KickClient => "KickClient",
        PartyPermission::ControlPlayback => "ControlPlayback",
        PartyPermission::SeekPlayback => "SeekPlayback",
        PartyPermission::ModifyQueue => "ModifyQueue",
        PartyPermission::SuggestTrack => "SuggestTrack",
        PartyPermission::RemoveOwnSuggestion => "RemoveOwnSuggestion",
        PartyPermission::RemoveAnyTrack => "RemoveAnyTrack",
        PartyPermission::VoteTrack => "VoteTrack",
        PartyPermission::VoteSkip => "VoteSkip",
        PartyPermission::ChangeRoomSettings => "ChangeRoomSettings",
        PartyPermission::ViewMembers => "ViewMembers",
        PartyPermission::SendChatMessage => "SendChatMessage",
    }
}

const ALL_PERMISSIONS: [PartyPermission; 22] = [
    PartyPermission::CreateRoom,
    PartyPermission::DiscoverRooms,
    PartyPermission::JoinRoom,
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
    PartyPermission::SuggestTrack,
    PartyPermission::RemoveOwnSuggestion,
    PartyPermission::RemoveAnyTrack,
    PartyPermission::VoteTrack,
    PartyPermission::VoteSkip,
    PartyPermission::ChangeRoomSettings,
    PartyPermission::ViewMembers,
    PartyPermission::SendChatMessage,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_youtube_and_protocol_version() {
        let output = render_command(Some(&Command::Status));

        assert!(output.contains("platform: YouTube"));
        assert!(output.contains("party protocol: v1"));
    }

    #[test]
    fn client_permissions_exclude_playback_control() {
        let output = render_permissions(Some(RoleArg::Client));

        assert!(output.contains("client:"));
        assert!(output.contains("SuggestTrack"));
        assert!(!output.contains("ControlPlayback"));
    }

    #[test]
    fn admin_permissions_include_admin_actions() {
        let output = render_permissions(Some(RoleArg::Admin));

        assert!(output.contains("CreateRoom"));
        assert!(output.contains("TransferAdmin"));
        assert!(output.contains("ControlPlayback"));
    }

    #[test]
    fn party_stubs_report_required_permission() {
        let output = render_party_command(&PartyCommand::Kick {
            client_id: "client-1".to_string(),
        });

        assert!(output.contains("kick client client-1"));
        assert!(output.contains("required permission: KickClient"));
        assert!(output.contains("network handler not implemented"));
    }

    #[test]
    fn snapshot_renders_room_members_queue_and_playback() {
        let output = render_snapshot(false, None, None);

        assert!(output.contains("room-demo"));
        assert!(output.contains("Demo Room"));
        assert!(output.contains("admin-1"));
        assert!(output.contains("client-bob"));
        assert!(output.contains("Members (2)"));
        assert!(output.contains("Queue (2 items)"));
        assert!(output.contains("Never Gonna Give You Up"));
        assert!(output.contains("GANGNAM STYLE"));
        assert!(output.contains("playing:  true"));
        assert!(output.contains("dQw4w9WgXcQ"));
    }

    #[test]
    fn snapshot_json_output_is_valid_json_with_expected_fields() {
        let output = render_snapshot(true, None, None);

        let value: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
        assert_eq!(value["room"]["room_id"], "room-demo");
        assert_eq!(value["room"]["room_name"], "Demo Room");
        assert_eq!(value["current_admin"], "admin-1");
        assert_eq!(value["protocol_version"], 1);
        assert_eq!(value["members"].as_array().unwrap().len(), 2);
        assert_eq!(value["queue"].as_array().unwrap().len(), 2);
        assert!(value["playback_state"]["is_playing"].as_bool().unwrap());
    }

    #[test]
    fn snapshot_filter_state_active_matches_demo_room() {
        let output = render_snapshot(false, Some(RoomStateArg::Active), None);
        assert!(output.contains("room-demo"));
        assert!(!output.contains("No rooms match"));
    }

    #[test]
    fn snapshot_filter_state_locked_excludes_active_demo_room() {
        let output = render_snapshot(false, Some(RoomStateArg::Locked), None);
        assert_eq!(output, "No rooms match the filter.\n");
    }

    #[test]
    fn snapshot_filter_min_members_two_passes() {
        let output = render_snapshot(false, None, Some(2));
        assert!(output.contains("room-demo"));
    }

    #[test]
    fn snapshot_filter_min_members_three_excludes() {
        let output = render_snapshot(false, None, Some(3));
        assert_eq!(output, "No rooms match the filter.\n");
    }

    #[test]
    fn snapshot_filter_paused_excludes_active_demo_room() {
        let output = render_snapshot(false, Some(RoomStateArg::Paused), None);
        assert_eq!(output, "No rooms match the filter.\n");
    }

    #[test]
    fn snapshot_filter_active_and_json_combined() {
        let output = render_snapshot(true, Some(RoomStateArg::Active), Some(2));
        let value: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
        assert_eq!(value["room"]["room_id"], "room-demo");
    }
}
