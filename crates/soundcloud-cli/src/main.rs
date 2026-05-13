use std::fmt::Write;

use clap::{Parser, Subcommand, ValueEnum};
use meowify_party::{PROTOCOL_VERSION, PartyPermission, PartyRole, can};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RoleArg {
    Admin,
    Moderator,
    Client,
    Guest,
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
}
