use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "soundcloud-cli")]
#[command(about = "Meowify debugging/admin CLI")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Party {
        #[command(subcommand)]
        command: PartyCommand,
    },
}

#[derive(Debug, Subcommand)]
enum PartyCommand {
    Create,
    Rooms,
    Join { room_id: String },
    JoinCode { code: String },
    Leave,
    Members,
    Approve { client_id: String },
    Reject { client_id: String },
    Promote { client_id: String },
    Demote { client_id: String },
    TransferAdmin { client_id: String },
    Kick { client_id: String },
    Lock,
    Unlock,
    End,
    Queue,
    Vote { queue_item_id: String },
    Suggest { track_ref: String },
    Permissions,
}

fn main() {
    let args = Args::parse();
    println!("{args:#?}");
}
