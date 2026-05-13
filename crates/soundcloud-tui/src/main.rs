use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use meowify_party::{
    ConnectionState, JoinRequest, LanDiscoveryHandle, PartyClient, PartyRole,
    PlaybackCommandKind, RoomServer, RoomVisibility, TrackRef,
};
use meowify_playback::gst::GstBackend;
use meowify_playback::{PlaybackError, PlaybackState, PlaybackStatus};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
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
            Self::Search => "Search tracks and artists in local library",
            Self::Library => "Local playlists, follows, favorites, and imported files",
            Self::Party => "LAN room state, queue, roles, permissions, and discovery",
            Self::Offline => "Imported local files and metadata — no account needed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    ImportPath,
}

struct AppState {
    selected: usize,
    should_quit: bool,
    playback: PlaybackState,
    room: RoomServer,
    last_event: String,
    discovery: Option<LanDiscoveryHandle>,
    input_mode: InputMode,
    input_buf: String,
    imported_count: usize,
    gst: Option<GstBackend>,
    volume: f64,
    disco_cursor: usize,
}

impl Default for AppState {
    fn default() -> Self {
        let gst = GstBackend::new().ok();
        Self {
            selected: 0,
            should_quit: false,
            playback: PlaybackState::default(),
            room: make_demo_room(),
            last_event: "ready — press i to import a local audio file".to_string(),
            discovery: None,
            input_mode: InputMode::Normal,
            input_buf: String::new(),
            imported_count: 0,
            gst,
            volume: 0.8,
            disco_cursor: 0,
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
        match self.input_mode {
            InputMode::ImportPath => match code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.input_buf.clear();
                    self.last_event = "import cancelled".to_string();
                }
                KeyCode::Enter => {
                    self.do_import();
                    self.input_mode = InputMode::Normal;
                    self.input_buf.clear();
                }
                KeyCode::Char(c) => {
                    self.input_buf.push(c);
                }
                KeyCode::Backspace => {
                    self.input_buf.pop();
                }
                _ => {}
            },
            InputMode::Normal => match code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.selected_view() == View::Party && self.discovery.is_some() {
                        let n = self
                            .discovery
                            .as_ref()
                            .map(|d| d.discovered_rooms().len())
                            .unwrap_or(0);
                        if n > 0 {
                            self.disco_cursor = (self.disco_cursor + 1) % n;
                            self.last_event =
                                format!("selected room {}/{}", self.disco_cursor + 1, n);
                            return;
                        }
                    }
                    self.next();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.selected_view() == View::Party && self.discovery.is_some() {
                        let n = self
                            .discovery
                            .as_ref()
                            .map(|d| d.discovered_rooms().len())
                            .unwrap_or(0);
                        if n > 0 {
                            self.disco_cursor = if self.disco_cursor == 0 {
                                n - 1
                            } else {
                                self.disco_cursor - 1
                            };
                            self.last_event =
                                format!("selected room {}/{}", self.disco_cursor + 1, n);
                            return;
                        }
                    }
                    self.previous();
                }
                KeyCode::Char(' ') => self.toggle_playback(),
                KeyCode::Char('s') => self.stop_playback(),
                KeyCode::Char('n') => self.skip_next(),
                KeyCode::Char('p') => self.skip_previous(),
                KeyCode::Char('d') => self.toggle_discovery(),
                KeyCode::Char('l') => self.lock_room(),
                KeyCode::Char('u') => self.unlock_room(),
                KeyCode::Char('e') => self.end_room(),
                KeyCode::Char('a') => self.approve_next_pending(),
                KeyCode::Char('r') => self.reject_next_pending(),
                KeyCode::Enter if self.selected_view() == View::Party => self.join_selected_room(),
                KeyCode::Char('+') | KeyCode::Char('=') => self.volume_up(),
                KeyCode::Char('-') | KeyCode::Char('_') => self.volume_down(),
                KeyCode::Char('i') => {
                    self.input_mode = InputMode::ImportPath;
                    self.input_buf.clear();
                    self.last_event =
                        "enter file path, press Enter to import, Esc to cancel".to_string();
                }
                _ => {}
            },
        }
    }

    fn volume_up(&mut self) {
        self.volume = (self.volume + 0.1).min(1.0);
        if let Some(ref gst) = self.gst {
            gst.set_volume(self.volume);
        }
        self.last_event = format!("volume: {:.0}%", self.volume * 100.0);
    }

    fn volume_down(&mut self) {
        self.volume = (self.volume - 0.1).max(0.0);
        if let Some(ref gst) = self.gst {
            gst.set_volume(self.volume);
        }
        self.last_event = format!("volume: {:.0}%", self.volume * 100.0);
    }

    fn join_selected_room(&mut self) {
        let rooms = match self.discovery {
            Some(ref d) => d.discovered_rooms(),
            None => {
                self.last_event = "no discovery running — press d to start".to_string();
                return;
            }
        };
        if rooms.is_empty() {
            self.last_event = "no rooms discovered yet".to_string();
            return;
        }
        if self.disco_cursor >= rooms.len() {
            self.disco_cursor = 0;
        }
        let room = &rooms[self.disco_cursor];
        let request_id = format!("req-join-{}", room.room_id);
        let _ = self.room.handle_join_request(JoinRequest {
            request_id: request_id.clone(),
            room_id: room.room_id.clone(),
            client_id: format!("client-{}", room.room_id),
            user_name: "LocalUser".to_string(),
            device_name: "meowify-tui".to_string(),
            invite_code_attempt: None,
            requested_at_ms: 9999,
        });
        self.last_event = format!("join request sent to room: {}", room.room_name);
    }

    fn do_import(&mut self) {
        let path = std::path::Path::new(&self.input_buf);
        if !path.exists() {
            self.last_event = format!("file not found: {}", self.input_buf);
            return;
        }
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let local_id = format!("local-import-{}", self.imported_count + 1);
        self.playback.queue.push(meowify_playback::QueueItem {
            id: local_id.clone(),
            source: meowify_playback::PlaybackSource::ImportedLocalFile {
                path: self.input_buf.clone(),
            },
            title: title.clone(),
        });
        self.imported_count += 1;
        self.last_event = format!("imported: {title}");
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

    fn toggle_discovery(&mut self) {
        if self.discovery.is_some() {
            self.discovery = None;
            self.last_event = "LAN discovery stopped".to_string();
        } else {
            match LanDiscoveryHandle::start() {
                Some(handle) => {
                    self.discovery = Some(handle);
                    self.last_event = "LAN discovery started".to_string();
                }
                None => {
                    self.last_event = "LAN discovery failed to start".to_string();
                }
            }
        }
    }

    fn approve_next_pending(&mut self) {
        let ids: Vec<String> = self
            .room
            .join_requests()
            .map(|r| r.request_id.clone())
            .collect();
        if ids.is_empty() {
            self.last_event = "no pending requests to approve".to_string();
            return;
        }
        self.last_event = match self
            .room
            .approve_join("admin-1", &ids[0], PartyRole::Client, 9999)
        {
            Ok(_) => format!("approved request: {}", ids[0]),
            Err(e) => format!("approve failed: {e}"),
        };
    }

    fn reject_next_pending(&mut self) {
        let ids: Vec<String> = self
            .room
            .join_requests()
            .map(|r| r.request_id.clone())
            .collect();
        if ids.is_empty() {
            self.last_event = "no pending requests to reject".to_string();
            return;
        }
        self.last_event = match self.room.reject_join("admin-1", &ids[0]) {
            Ok(_) => format!("rejected request: {}", ids[0]),
            Err(e) => format!("reject failed: {e}"),
        };
    }

    fn toggle_playback(&mut self) {
        match self.playback.status {
            PlaybackStatus::Playing => {
                self.playback.pause();
                self.last_event = "playback paused".to_string();
                if let Some(ref gst) = self.gst {
                    let _ = gst.pause();
                }
            }
            PlaybackStatus::Stopped | PlaybackStatus::Paused => match self.playback.play() {
                Ok(()) => {
                    self.last_event = "playback started".to_string();
                    self.play_current_gst();
                }
                Err(PlaybackError::QueueEmpty) => {
                    self.last_event = "queue empty — press i to import a file".to_string();
                }
            },
        }
    }

    fn play_current_gst(&mut self) {
        let item = match self.playback.current() {
            Some(i) => i.clone(),
            None => return,
        };
        let (path, uri) = match &item.source {
            meowify_playback::PlaybackSource::ImportedLocalFile { path } => {
                (path.clone(), format!("file://{}", path))
            }
            meowify_playback::PlaybackSource::YouTubeVideo { .. }
            | meowify_playback::PlaybackSource::SoundCloudTrack { .. } => return,
        };
        if !std::path::Path::new(&path).exists() {
            self.last_event = format!("file not found: {path}");
            return;
        }
        if let Some(ref gst) = self.gst {
            let _ = gst.set_uri(&uri);
            let _ = gst.play();
        }
    }

    fn stop_playback(&mut self) {
        self.playback.stop();
        self.last_event = "playback stopped".to_string();
        if let Some(ref gst) = self.gst {
            let _ = gst.stop();
        }
    }

    fn skip_next(&mut self) {
        if let Some(ref gst) = self.gst {
            let _ = gst.stop();
        }
        self.last_event = match self.playback.skip_next().map(|item| item.title.clone()) {
            Some(title) => {
                self.play_current_gst();
                format!("skipped to next: {title}")
            }
            None => "no next item; playback stopped".to_string(),
        };
    }

    fn skip_previous(&mut self) {
        if let Some(ref gst) = self.gst {
            let _ = gst.stop();
        }
        self.last_event = match self.playback.skip_previous().map(|item| item.title.clone()) {
            Some(title) => {
                self.play_current_gst();
                format!("skipped to previous: {title}")
            }
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
    let _ = server.handle_join_request(JoinRequest {
        request_id: "req-carol".to_string(),
        room_id: "demo-room".to_string(),
        client_id: "client-carol".to_string(),
        user_name: "Carol".to_string(),
        device_name: "tablet".to_string(),
        invite_code_attempt: None,
        requested_at_ms: 1500,
    });
    let _ = server.add_queue_item(
        "admin-1",
        "item-1",
        TrackRef::YouTube {
            video_id: "dQw4w9WgXcQ".to_string(),
            title: Some("Never Gonna Give You Up".to_string()),
            channel_title: Some("Rick Astley".to_string()),
        },
    );
    let _ = server.add_queue_item(
        "admin-1",
        "item-2",
        TrackRef::SoundCloud {
            track_id: "12345".to_string(),
            title: Some("SC Demo Track".to_string()),
            user_title: Some("Demo Artist".to_string()),
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
        if app.playback.status == PlaybackStatus::Playing {
            if let Some(ref gst) = app.gst {
                if let Some(pos) = gst.position() {
                    app.playback.position_ms = pos.as_millis() as u64;
                }
            }
        }

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

    if app.selected_view() == View::Party {
        let (party_layout, progress_extra) =
            if app.playback.status == PlaybackStatus::Playing && app.gst.is_some() {
                let c = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(3)])
                    .split(layout[1]);
                (c[0], Some(c[1]))
            } else {
                (layout[1], None)
            };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(14),
                Constraint::Min(5),
                Constraint::Length(8),
            ])
            .split(party_layout);

        let snap = app.room.snapshot();
        frame.render_widget(party_header(app, &snap), chunks[0]);
        frame.render_widget(party_center_panel(app, &snap), chunks[1]);
        frame.render_widget(party_queue_widget(&snap), chunks[2]);
        if let Some(progress_area) = progress_extra {
            render_progress_bar(frame, progress_area, app);
        }
    } else if app.playback.status == PlaybackStatus::Playing {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(layout[1]);
        frame.render_widget(detail_panel(app), chunks[0]);
        render_progress_bar(frame, chunks[1], app);
    } else {
        frame.render_widget(detail_panel(app), layout[1]);
    }
}

fn render_progress_bar(frame: &mut Frame, area: ratatui::layout::Rect, app: &AppState) {
    let gst = match app.gst {
        Some(ref g) => g,
        None => return,
    };
    let dur = match gst.duration() {
        Some(d) => d,
        None => return,
    };
    let total_ms = dur.as_millis() as u64;
    if total_ms == 0 {
        return;
    }
    let pct = app.playback.position_ms as f64 / total_ms as f64 * 100.0;
    let label = format!(
        " {} / {} ",
        ms_fmt(app.playback.position_ms),
        ms_fmt(total_ms)
    );
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Progress "))
        .gauge_style(Style::default().fg(Color::Cyan))
        .percent(pct as u16)
        .label(label);
    frame.render_widget(gauge, area);
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
    let offline_policy = "Offline mode: local files and metadata — no account required";

    let keys = match app.input_mode {
        InputMode::Normal => {
            "Keys: i import, +/- vol, space play/pause, s stop, n/p skip, j/k nav, q quit"
        }
        InputMode::ImportPath => "Type path, Enter confirm, Esc cancel",
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
        Line::from(keys),
    ];

    if app.input_mode == InputMode::ImportPath {
        let prompt = format!("Import path: {}", app.input_buf);
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            prompt,
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(""));

        return Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Current view "),
            )
            .wrap(Wrap { trim: true });
    }

    lines.push(Line::from(""));
    lines.push(Line::from(playback_status_line(&app.playback)));
    lines.push(Line::from(playback_queue_line(&app.playback)));
    lines.push(Line::from(playback_current_line(&app.playback)));
    lines.push(Line::from(format!("Volume: {:.0}%", app.volume * 100.0)));

    if app.playback.status == PlaybackStatus::Playing && app.gst.is_some() {
        if let Some(ref gst) = app.gst {
            if let Some(dur) = gst.duration() {
                let total = dur.as_millis() as u64;
                if total > 0 {
                    lines.push(Line::from(format!(
                        "Progress: {}/{}",
                        ms_fmt(app.playback.position_ms),
                        ms_fmt(total)
                    )));
                }
            }
        }
    }
    lines.push(Line::from(format!("Last event: {}", app.last_event)));
    lines.push(Line::from(""));
    lines.push(Line::from(offline_policy));

    if selected == View::Party {
        let snap = app.room.snapshot();
        lines.push(Line::from(""));
        lines.push(Line::from(party_room_line(&snap)));
        lines.push(Line::from(party_playback_line(&snap)));
        lines.push(Line::from(""));
        lines.push(Line::from(format!("Members ({}):", snap.members.len())));
        for row in party_member_rows(&snap) {
            lines.push(Line::from(row));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(format!("Queue ({} item(s)):", snap.queue.len())));
        if snap.queue.is_empty() {
            lines.push(Line::from("  (empty)"));
        }
        for row in party_queue_rows(&snap) {
            lines.push(Line::from(row));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("Party keys: l lock, u unlock, e end room"));
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

fn ms_fmt(ms: u64) -> String {
    let secs = ms / 1000;
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
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
        "Room: {} | State: {:?} | Protocol v{} | Visibility: {:?}",
        snap.room.room_name, snap.room.state, snap.protocol_version, snap.room.visibility
    )
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
        Some(TrackRef::SoundCloud {
            title, user_title, ..
        }) => format!(
            "Now playing: {} — {} (SC) at {} ms",
            title.as_deref().unwrap_or("(no title)"),
            user_title.as_deref().unwrap_or("(no user)"),
            pb.position_ms
        ),
        Some(TrackRef::ImportedLocalFile { title, .. }) => {
            format!("Now playing: [local] {title} at {} ms", pb.position_ms)
        }
        None => "Playback: idle".to_string(),
    }
}

fn party_header(app: &AppState, snap: &meowify_party::RoomSnapshot) -> Paragraph<'static> {
    let pb = &snap.playback_state;
    let offline_policy = "Offline mode: local files and metadata — no account required";

    let lines = vec![
        Line::from(Span::styled(
            "Party",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Admin: {}", snap.current_admin)),
        Line::from(party_room_line(snap)),
        Line::from(party_playback_line(snap)),
        Line::from(format!(
            "Seq: {} | Position: {} ms",
            pb.sequence_number, pb.position_ms
        )),
        Line::from(""),
        Line::from(format!("Last event: {}", app.last_event)),
        Line::from(offline_policy),
        Line::from(""),
        Line::from(
            "Keys: d discover, i import, +/- vol, l lock, u unlock, e end, a approve, r reject, j/k nav, space play/pause, s stop, n/p skip, q quit",
        ),
    ];

    Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Party "))
}

fn party_member_rows(snap: &meowify_party::RoomSnapshot) -> Vec<String> {
    let mut members = snap.members.clone();
    members.sort_by(|a, b| a.client_id.cmp(&b.client_id));
    members
        .iter()
        .map(|m| format!("  {:?} | {} | {}", m.role, m.client_id, m.user_name))
        .collect()
}

fn party_queue_rows(snap: &meowify_party::RoomSnapshot) -> Vec<String> {
    snap.queue
        .iter()
        .map(|item| {
            let title = match &item.track_ref {
                TrackRef::YouTube {
                    title,
                    channel_title,
                    ..
                } => format!(
                    "{} — {}",
                    title.as_deref().unwrap_or("(no title)"),
                    channel_title.as_deref().unwrap_or("(no channel)")
                ),
                TrackRef::SoundCloud {
                    title, user_title, ..
                } => format!(
                    "{} — {}",
                    title.as_deref().unwrap_or("(no title)"),
                    user_title.as_deref().unwrap_or("(no user)")
                ),
                TrackRef::ImportedLocalFile { title, .. } => format!("[local] {title}"),
            };
            format!(
                "  {} | votes:{:+} | {}",
                item.queue_item_id, item.votes, title
            )
        })
        .collect()
}

fn role_style(role: PartyRole) -> Style {
    match role {
        PartyRole::Admin => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        PartyRole::Moderator => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        PartyRole::Client => Style::default().fg(Color::White),
        PartyRole::Guest => Style::default().fg(Color::DarkGray),
    }
}

fn party_center_panel(app: &AppState, snap: &meowify_party::RoomSnapshot) -> List<'static> {
    let mut items: Vec<ListItem<'static>> = Vec::new();

    if let Some(discovery) = &app.discovery {
        let rooms = discovery.discovered_rooms();
        let n = rooms.len();
        let disc_header = format!("Discovered rooms ({n}):");
        items.push(
            ListItem::new(Line::from(Span::styled(
                disc_header,
                Style::default().add_modifier(Modifier::BOLD),
            )))
            .style(Style::default()),
        );
        for (i, room) in rooms.iter().enumerate() {
            let name = room.room_name.clone();
            let vis = format!("{:?}", room.visibility);
            let is_selected = i == app.disco_cursor;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green)
            };
            let prefix = if is_selected { " >" } else { "  " };
            items.push(
                ListItem::new(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(" ", Style::default()),
                    Span::styled(name, style),
                    Span::raw(format!(" ({vis})")),
                ]))
                .style(style),
            );
        }
        if n > 0 {
            items.push(ListItem::new(Line::from(Span::styled(
                "  Enter to join, j/k to navigate",
                Style::default().fg(Color::DarkGray),
            ))));
        }
        items.push(ListItem::new(Line::from("")));
    } else {
        items.push(
            ListItem::new(Line::from(Span::styled(
                "LAN discovery off — press d to start",
                Style::default().fg(Color::DarkGray),
            )))
            .style(Style::default()),
        );
        items.push(ListItem::new(Line::from("")));
    }

    let mut members = snap.members.clone();
    members.sort_by(|a, b| a.client_id.cmp(&b.client_id));
    let member_header = format!("Members ({}):", members.len());
    items.push(
        ListItem::new(Line::from(Span::styled(
            member_header.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )))
        .style(Style::default()),
    );
    for m in &members {
        let role_label = format!("{:?}", m.role);
        let text = format!("  {} | {}", m.user_name, m.client_id);
        let style = role_style(m.role);
        items.push(
            ListItem::new(Line::from(vec![
                Span::styled(role_label, style),
                Span::raw(" "),
                Span::raw(text),
            ]))
            .style(style),
        );
    }

    let pending = &snap.pending_requests;
    if !pending.is_empty() {
        items.push(ListItem::new(Line::from("")));
        let pending_header = format!("Pending requests ({}):", pending.len());
        items.push(
            ListItem::new(Line::from(Span::styled(
                pending_header,
                Style::default().add_modifier(Modifier::BOLD),
            )))
            .style(Style::default()),
        );
        for req in pending {
            let code = req.invite_code_attempt.as_deref().unwrap_or("no code");
            items.push(
                ListItem::new(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(req.user_name.clone(), Style::default().fg(Color::Cyan)),
                    Span::raw(format!(" ({}) — {}", req.device_name, code)),
                ]))
                .style(Style::default()),
            );
        }
    }

    List::new(items).block(Block::default().borders(Borders::ALL).title(" Room state "))
}

fn party_queue_widget(snap: &meowify_party::RoomSnapshot) -> Paragraph<'static> {
    let queue = &snap.queue;
    let header = format!("Queue ({} item(s)):", queue.len());
    let mut lines = vec![Line::from(Span::styled(
        header,
        Style::default().add_modifier(Modifier::BOLD),
    ))];

    if queue.is_empty() {
        lines.push(Line::from("  (empty)"));
    }
    for item in queue {
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
            TrackRef::SoundCloud {
                title, user_title, ..
            } => format!(
                "{} — {}",
                title.as_deref().unwrap_or("(no title)"),
                user_title.as_deref().unwrap_or("(no user)")
            ),
            TrackRef::ImportedLocalFile { title, .. } => format!("[local] {title}"),
        };
        lines.push(Line::from(format!(
            "  {} | votes:{:+} | by:{} | {}",
            item.queue_item_id, item.votes, item.suggested_by, track_label
        )));
    }

    Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Queue "))
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
    fn tui_guardrails_match_party_rbac_policy() {
        assert!(!can(PartyRole::Client, PartyPermission::ControlPlayback));
    }

    #[test]
    fn party_view_shows_demo_room_state() {
        let app = AppState::default();
        let snap = app.room.snapshot();

        let room_line = party_room_line(&snap);
        assert!(room_line.contains("LAN Party Demo"));
        assert!(room_line.contains("Protocol v"));

        let member_rows = party_member_rows(&snap);
        assert_eq!(member_rows.len(), 2);
        assert!(member_rows.iter().any(|r| r.contains("Alice")));
        assert!(member_rows.iter().any(|r| r.contains("Bob")));

        let queue_rows = party_queue_rows(&snap);
        assert!(!queue_rows.is_empty());
        assert!(
            queue_rows
                .iter()
                .any(|r| r.contains("Never Gonna Give You Up"))
        );
        assert!(queue_rows.iter().any(|r| r.contains("Rick Astley")));
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
    fn approve_key_accepts_first_pending_request() {
        let mut app = AppState::default();
        let n_initial = app.room.snapshot().members.len();

        app.handle_key(KeyCode::Char('a'));

        let snap = app.room.snapshot();
        assert_eq!(snap.members.len(), n_initial + 1);
        assert!(app.last_event.starts_with("approved request:"));
    }

    #[test]
    fn reject_key_removes_pending_request() {
        let mut app = AppState::default();
        let n_before = app.room.snapshot().pending_requests.len();
        assert!(n_before > 0, "demo room should have pending requests");

        app.handle_key(KeyCode::Char('r'));

        assert!(app.last_event.starts_with("rejected request:"));
        assert_eq!(app.room.snapshot().pending_requests.len(), n_before - 1);
    }

    #[test]
    fn approve_with_no_pending_reports_appropriate_message() {
        let admin = PartyClient {
            client_id: "admin-1".to_string(),
            device_name: "laptop".to_string(),
            user_name: "Admin".to_string(),
            role: PartyRole::Admin,
            permissions_override: Vec::new(),
            connected_at_ms: 0,
            last_seen_ms: 0,
            connection_state: ConnectionState::Connected,
        };
        let server = RoomServer::create(
            "empty-room",
            "No Pending",
            RoomVisibility::LanVisible,
            admin,
            "inv",
            0,
        );
        let mut app = AppState {
            room: server,
            ..AppState::default()
        };
        assert_eq!(app.room.snapshot().pending_requests.len(), 0);

        app.approve_next_pending();

        assert_eq!(app.last_event, "no pending requests to approve");
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
        let mut app = AppState {
            gst: None,
            ..AppState::default()
        };

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

    #[test]
    fn party_header_includes_admin_room_playback_seq_and_keys() {
        let app = AppState::default();
        let snap = app.room.snapshot();

        let para = party_header(&app, &snap);
        let content = format!("{para:?}");

        assert!(content.contains("admin-1"));
        assert!(content.contains("LAN Party Demo"));
        assert!(content.contains("Protocol v"));
        assert!(content.contains("position"));
        assert!(content.contains("l lock"));
        assert!(content.contains("e end"));
    }

    #[test]
    fn party_center_panel_lists_members_with_role_styles() {
        let app = AppState::default();
        let snap = app.room.snapshot();

        let list = party_center_panel(&app, &snap);
        let content = format!("{list:?}");

        assert!(content.contains("Members (2)"));
        assert!(content.contains("Alice"));
        assert!(content.contains("Bob"));
        assert!(content.contains("Admin"));
        assert!(content.contains("Client"));
    }

    #[test]
    fn party_center_panel_shows_pending_requests_when_present() {
        let server = make_demo_room();
        let snap = server.snapshot();
        let app = AppState::default();

        let list = party_center_panel(&app, &snap);
        let content = format!("{list:?}");

        assert!(content.contains("Pending requests (1)"));
        assert!(content.contains("Carol"));
        assert!(content.contains("tablet"));
    }

    #[test]
    fn party_center_panel_shows_discovery_off_by_default() {
        let app = AppState::default();
        let snap = app.room.snapshot();

        let list = party_center_panel(&app, &snap);
        let content = format!("{list:?}");

        assert!(content.contains("LAN discovery off"));
    }

    #[test]
    fn import_key_enters_import_mode() {
        let mut app = AppState::default();
        assert_eq!(app.input_mode, InputMode::Normal);

        app.handle_key(KeyCode::Char('i'));

        assert_eq!(app.input_mode, InputMode::ImportPath);
        assert!(app.last_event.contains("enter file path"));
    }

    #[test]
    fn import_mode_esc_returns_to_normal() {
        let mut app = AppState::default();
        app.handle_key(KeyCode::Char('i'));
        app.handle_key(KeyCode::Esc);

        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.last_event, "import cancelled");
    }

    #[test]
    fn import_non_existent_file_reports_error() {
        let mut app = AppState {
            input_mode: InputMode::ImportPath,
            input_buf: "/nonexistent/path.flac".to_string(),
            ..AppState::default()
        };
        app.handle_key(KeyCode::Enter);

        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.last_event.contains("file not found"));
    }

    #[test]
    fn volume_up_increases_volume() {
        let mut app = AppState::default();
        let initial = app.volume;
        app.volume_up();
        assert!((app.volume - initial - 0.1).abs() < 0.001);
        assert!(app.last_event.contains("volume:"));
    }

    #[test]
    fn volume_down_decreases_volume() {
        let mut app = AppState {
            volume: 0.5,
            ..AppState::default()
        };
        app.volume_down();
        assert!((app.volume - 0.4).abs() < 0.001);
    }

    #[test]
    fn volume_clamps_at_zero() {
        let mut app = AppState {
            volume: 0.0,
            ..AppState::default()
        };
        app.volume_down();
        assert!((app.volume - 0.0).abs() < 0.001);
    }

    #[test]
    fn volume_clamps_at_one() {
        let mut app = AppState {
            volume: 1.0,
            ..AppState::default()
        };
        app.volume_up();
        assert!((app.volume - 1.0).abs() < 0.001);
    }

    #[test]
    fn ms_fmt_formats_correctly() {
        assert_eq!(ms_fmt(0), "0:00");
        assert_eq!(ms_fmt(5_000), "0:05");
        assert_eq!(ms_fmt(65_000), "1:05");
        assert_eq!(ms_fmt(3_600_000), "60:00");
    }

    #[test]
    fn toggle_discovery_starts_or_reports_failure() {
        let mut app = AppState::default();
        assert!(app.discovery.is_none());

        app.toggle_discovery();
        if app.discovery.is_some() {
            assert!(app.last_event.contains("started"));
            app.toggle_discovery();
            assert!(app.discovery.is_none());
            assert!(app.last_event.contains("stopped"));
        } else {
            assert!(
                app.last_event.contains("failed"),
                "expected failure in test env: {}",
                app.last_event
            );
        }
    }

    #[test]
    fn party_queue_widget_shows_items_and_votes() {
        let app = AppState::default();
        let snap = app.room.snapshot();

        let para = party_queue_widget(&snap);
        let content = format!("{para:?}");

        assert!(content.contains("Queue"));
        assert!(content.contains("Never Gonna Give You Up"));
        assert!(content.contains("Rick Astley"));
        assert!(content.contains("SC Demo Track"));
    }

    #[test]
    fn party_queue_widget_shows_empty_when_no_items() {
        let admin = PartyClient {
            client_id: "admin-1".to_string(),
            device_name: "laptop".to_string(),
            user_name: "Admin".to_string(),
            role: PartyRole::Admin,
            permissions_override: Vec::new(),
            connected_at_ms: 0,
            last_seen_ms: 0,
            connection_state: ConnectionState::Connected,
        };
        let server = RoomServer::create(
            "r1",
            "Empty Room",
            RoomVisibility::LanVisible,
            admin,
            "inv",
            0,
        );
        let snap = server.snapshot();

        let para = party_queue_widget(&snap);
        let content = format!("{para:?}");

        assert!(content.contains("(empty)"));
    }

    #[test]
    fn role_style_assigns_distinct_styles_per_role() {
        assert_eq!(
            role_style(PartyRole::Admin),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            role_style(PartyRole::Client),
            Style::default().fg(Color::White)
        );
        assert_eq!(
            role_style(PartyRole::Guest),
            Style::default().fg(Color::DarkGray)
        );
    }
}
