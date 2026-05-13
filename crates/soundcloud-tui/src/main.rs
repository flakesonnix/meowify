use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use meowify_core::can_persist_youtube_audio;
use meowify_party::{
    ConnectionState, JoinRequest, PartyClient, PartyRole, PlaybackCommandKind, RoomServer,
    RoomVisibility, TrackRef,
};
use meowify_playback::{PlaybackError, PlaybackState, PlaybackStatus};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Home,
    Search,
    Library,
    Party,
    Offline,
}

impl View {
    const ALL: [Self; 5] = [
        Self::Home,
        Self::Search,
        Self::Library,
        Self::Party,
        Self::Offline,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Search => "Search",
            Self::Library => "Library",
            Self::Party => "Party",
            Self::Offline => "Offline",
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Self::Home => "Status and setup checklist",
            Self::Search => "YouTube Data API v3 search after OAuth setup",
            Self::Library => "Local playlists, follows, favorites, and imports",
            Self::Party => "LAN room state, queue, roles, and permissions",
            Self::Offline => "Local files and metadata refs; no YouTube audio persistence",
        }
    }
}

#[derive(Debug)]
struct AppState {
    selected: usize,
    should_quit: bool,
    playback: PlaybackState,
    room: RoomServer,
    last_event: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            selected: 0,
            should_quit: false,
            playback: PlaybackState::default(),
            room: make_demo_room(),
            last_event: "ready; queue empty until search/import wiring lands".to_string(),
        }
    }
}

impl AppState {
    fn selected_view(&self) -> View {
        View::ALL[self.selected]
    }

    fn next(&mut self) {
        self.selected = (self.selected + 1) % View::ALL.len();
    }

    fn previous(&mut self) {
        self.selected = if self.selected == 0 {
            View::ALL.len() - 1
        } else {
            self.selected - 1
        };
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.next(),
            KeyCode::Up | KeyCode::Char('k') => self.previous(),
            KeyCode::Char(' ') => self.toggle_playback(),
            KeyCode::Char('s') => self.stop_playback(),
            KeyCode::Char('n') => self.skip_next(),
            KeyCode::Char('p') => self.skip_previous(),
            KeyCode::Char('l') => self.lock_room(),
            KeyCode::Char('u') => self.unlock_room(),
            KeyCode::Char('e') => self.end_room(),
            _ => {}
        }
    }

    fn lock_room(&mut self) {
        self.last_event = match self.room.lock_room("admin-1") {
            Ok(()) => "room locked".to_string(),
            Err(e) => format!("lock failed: {e}"),
        };
    }

    fn unlock_room(&mut self) {
        self.last_event = match self.room.unlock_room("admin-1") {
            Ok(()) => "room unlocked".to_string(),
            Err(e) => format!("unlock failed: {e}"),
        };
    }

    fn end_room(&mut self) {
        self.last_event = match self.room.end_room("admin-1") {
            Ok(()) => "room ended".to_string(),
            Err(e) => format!("end failed: {e}"),
        };
    }

    fn toggle_playback(&mut self) {
        match self.playback.status {
            PlaybackStatus::Playing => {
                self.playback.pause();
                self.last_event = "playback paused".to_string();
            }
            PlaybackStatus::Stopped | PlaybackStatus::Paused => match self.playback.play() {
                Ok(()) => self.last_event = "playback started".to_string(),
                Err(PlaybackError::QueueEmpty) => {
                    self.last_event =
                        "queue empty; add tracks after search/import wiring lands".to_string();
                }
            },
        }
    }

    fn stop_playback(&mut self) {
        self.playback.stop();
        self.last_event = "playback stopped".to_string();
    }

    fn skip_next(&mut self) {
        self.last_event = match self.playback.skip_next().map(|item| item.title.clone()) {
            Some(title) => format!("skipped to next: {title}"),
            None => "no next item; playback stopped".to_string(),
        };
    }

    fn skip_previous(&mut self) {
        self.last_event = match self.playback.skip_previous().map(|item| item.title.clone()) {
            Some(title) => format!("skipped to previous: {title}"),
            None => "no previous item; playback stopped".to_string(),
        };
    }
}

fn make_demo_room() -> RoomServer {
    let admin = PartyClient {
        client_id: "admin-1".to_string(),
        device_name: "laptop".to_string(),
        user_name: "Alice (admin)".to_string(),
        role: PartyRole::Admin,
        permissions_override: Vec::new(),
        connected_at_ms: 0,
        last_seen_ms: 0,
        connection_state: ConnectionState::Connected,
    };
    let mut server = RoomServer::create(
        "demo-room",
        "LAN Party Demo",
        RoomVisibility::LanVisible,
        admin,
        "demo-invite",
        0,
    );
    let _ = server.handle_join_request(JoinRequest {
        request_id: "req-bob".to_string(),
        room_id: "demo-room".to_string(),
        client_id: "client-bob".to_string(),
        user_name: "Bob".to_string(),
        device_name: "phone".to_string(),
        invite_code_attempt: None,
        requested_at_ms: 500,
    });
    let _ = server.approve_join("admin-1", "req-bob", PartyRole::Client, 1000);
    let _ = server.add_queue_item(
        "admin-1",
        "item-1",
        TrackRef::YouTube {
            video_id: "dQw4w9WgXcQ".to_string(),
            title: Some("Never Gonna Give You Up".to_string()),
            channel_title: Some("Rick Astley".to_string()),
        },
    );
    let _ = server.apply_playback_command(
        "admin-1",
        PlaybackCommandKind::SetTrack {
            track_ref: TrackRef::YouTube {
                video_id: "dQw4w9WgXcQ".to_string(),
                title: Some("Never Gonna Give You Up".to_string()),
                channel_title: Some("Rick Astley".to_string()),
            },
        },
        2000,
    );
    server
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = AppState::default();

    while !app.should_quit {
        terminal.draw(|frame| render(frame, &app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code);
                }
            }
        }
    }

    Ok(())
}

fn render(frame: &mut Frame, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(50)])
        .split(frame.area());

    frame.render_widget(navigation(app), layout[0]);
    frame.render_widget(detail_panel(app), layout[1]);
}

fn navigation(app: &AppState) -> List<'static> {
    let items = View::ALL.into_iter().enumerate().map(|(index, view)| {
        let style = if index == app.selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        ListItem::new(Line::from(vec![Span::styled(view.title(), style)])).style(style)
    });

    List::new(items).block(Block::default().borders(Borders::ALL).title(" Meowify "))
}

fn detail_panel(app: &AppState) -> Paragraph<'static> {
    let selected = app.selected_view();
    let offline_policy = if can_persist_youtube_audio() {
        "YouTube audio persistence: enabled, verify explicit rights before use"
    } else {
        "YouTube audio persistence: disabled; use local imports and metadata refs"
    };

    let mut lines = vec![
        Line::from(Span::styled(
            selected.title(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(selected.detail()),
        Line::from(""),
        Line::from(
            "Keys: j/down next view, k/up previous view, space play/pause, s stop, n/p skip, q/esc quit",
        ),
        Line::from(""),
        Line::from(playback_status_line(&app.playback)),
        Line::from(playback_queue_line(&app.playback)),
        Line::from(playback_current_line(&app.playback)),
        Line::from(format!("Last event: {}", app.last_event)),
        Line::from(""),
        Line::from(offline_policy),
    ];

    if selected == View::Party {
        let snap = app.room.snapshot();
        lines.push(Line::from(""));
        lines.push(Line::from(party_room_line(&snap)));
        lines.push(Line::from(party_members_line(&snap)));
        lines.push(Line::from(party_queue_line(&snap)));
        lines.push(Line::from(party_playback_line(&snap)));
        lines.push(Line::from(
            "Party keys: l lock room, u unlock room, e end room",
        ));
    }

    Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Current view "),
        )
        .wrap(Wrap { trim: true })
}

fn playback_status_line(playback: &PlaybackState) -> String {
    format!(
        "Playback: {} at {} ms",
        playback_status_name(playback.status),
        playback.position_ms
    )
}

fn playback_queue_line(playback: &PlaybackState) -> String {
    format!("Queue items: {}", playback.queue.len())
}

fn playback_current_line(playback: &PlaybackState) -> String {
    playback
        .current()
        .map(|item| format!("Current: {}", item.title))
        .unwrap_or_else(|| "Current: none".to_string())
}

fn party_room_line(snap: &meowify_party::RoomSnapshot) -> String {
    format!(
        "Room: {} | State: {:?} | Protocol v{}",
        snap.room.room_name, snap.room.state, snap.protocol_version
    )
}

fn party_members_line(snap: &meowify_party::RoomSnapshot) -> String {
    format!("Members: {}", snap.members.len())
}

fn party_queue_line(snap: &meowify_party::RoomSnapshot) -> String {
    format!("Queue: {} item(s)", snap.queue.len())
}

fn party_playback_line(snap: &meowify_party::RoomSnapshot) -> String {
    let pb = &snap.playback_state;
    match &pb.track_ref {
        Some(TrackRef::YouTube {
            title, video_id, ..
        }) => format!(
            "Now playing: {} ({}) at {} ms",
            title.as_deref().unwrap_or("(no title)"),
            video_id,
            pb.position_ms
        ),
        Some(TrackRef::ImportedLocalFile { title, .. }) => {
            format!("Now playing: [local] {title} at {} ms", pb.position_ms)
        }
        None => "Playback: idle".to_string(),
    }
}

fn playback_status_name(status: PlaybackStatus) -> &'static str {
    match status {
        PlaybackStatus::Stopped => "stopped",
        PlaybackStatus::Playing => "playing",
        PlaybackStatus::Paused => "paused",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meowify_party::{PartyPermission, can};

    #[test]
    fn navigation_wraps_in_both_directions() {
        let mut app = AppState::default();

        app.previous();
        assert_eq!(app.selected_view(), View::Offline);

        app.next();
        assert_eq!(app.selected_view(), View::Home);
    }

    #[test]
    fn quit_keys_stop_event_loop() {
        let mut app = AppState::default();

        app.handle_key(KeyCode::Char('q'));

        assert!(app.should_quit);
    }

    #[test]
    fn tui_guardrails_match_core_and_party_policy() {
        assert!(!can_persist_youtube_audio());
        assert!(!can(PartyRole::Client, PartyPermission::ControlPlayback));
    }

    #[test]
    fn party_view_shows_demo_room_state() {
        let app = AppState::default();
        let snap = app.room.snapshot();

        let room_line = party_room_line(&snap);
        assert!(room_line.contains("LAN Party Demo"));
        assert!(room_line.contains("Protocol v"));

        let members_line = party_members_line(&snap);
        assert!(members_line.contains('2'));

        let queue_line = party_queue_line(&snap);
        assert!(queue_line.contains('1'));
    }

    #[test]
    fn lock_key_updates_room_state_and_last_event() {
        let mut app = AppState::default();
        app.handle_key(KeyCode::Char('l'));

        assert_eq!(app.last_event, "room locked");
        let snap = app.room.snapshot();
        assert!(matches!(snap.room.state, meowify_party::RoomState::Locked));
    }

    #[test]
    fn end_key_transitions_room_to_ended() {
        let mut app = AppState::default();
        app.handle_key(KeyCode::Char('e'));

        assert_eq!(app.last_event, "room ended");
        let snap = app.room.snapshot();
        assert!(matches!(snap.room.state, meowify_party::RoomState::Ended));
    }

    #[test]
    fn default_tui_state_reports_empty_playback_queue() {
        let app = AppState::default();

        assert_eq!(
            playback_status_line(&app.playback),
            "Playback: stopped at 0 ms"
        );
        assert_eq!(playback_queue_line(&app.playback), "Queue items: 0");
        assert_eq!(playback_current_line(&app.playback), "Current: none");
    }

    #[test]
    fn play_key_reports_empty_queue_without_panicking() {
        let mut app = AppState::default();

        app.handle_key(KeyCode::Char(' '));

        assert_eq!(app.playback.status, PlaybackStatus::Stopped);
        assert!(app.last_event.contains("queue empty"));
    }

    #[test]
    fn stop_key_resets_playback_status_line() {
        let mut app = AppState::default();
        app.playback.position_ms = 30_000;

        app.handle_key(KeyCode::Char('s'));

        assert_eq!(
            playback_status_line(&app.playback),
            "Playback: stopped at 0 ms"
        );
        assert_eq!(app.last_event, "playback stopped");
    }
}
