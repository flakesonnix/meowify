use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::glib;
use meowify_core::can_persist_youtube_audio;
use meowify_party::{
    ConnectionState, JoinRequest, LanDiscoveryHandle, PartyClient, PartyRole, PlaybackCommandKind,
    RoomServer, RoomSnapshot, RoomVisibility, TrackRef,
};
use meowify_playback::{PlaybackError, PlaybackState, PlaybackStatus};

const APP_ID: &str = "dev.meowify.Meowify";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShellSection {
    title: &'static str,
    detail: &'static str,
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Meowify")
        .default_width(1040)
        .default_height(680)
        .build();

    let header_bar = adw::HeaderBar::new();
    let title = gtk::Label::new(Some("Meowify"));
    title.add_css_class("title-2");
    header_bar.set_title_widget(Some(&title));

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header_bar);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.add_css_class("background");
    content.set_vexpand(true);
    content.append(&navigation_sidebar());
    content.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    content.append(&main_panel());
    root.append(&content);

    window.set_content(Some(&root));
    window.present();
}

fn navigation_sidebar() -> gtk::ScrolledWindow {
    let sidebar = gtk::ListBox::new();
    sidebar.set_selection_mode(gtk::SelectionMode::Single);
    sidebar.add_css_class("navigation-sidebar");

    for section in shell_sections() {
        let row = gtk::ListBoxRow::new();
        let row_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);

        let title = gtk::Label::new(Some(section.title));
        title.set_xalign(0.0);
        title.add_css_class("heading");

        let detail = gtk::Label::new(Some(section.detail));
        detail.set_xalign(0.0);
        detail.set_wrap(true);
        detail.add_css_class("dim-label");

        row_box.append(&title);
        row_box.append(&detail);
        row.set_child(Some(&row_box));
        sidebar.append(&row);
    }

    sidebar.select_row(sidebar.row_at_index(0).as_ref());

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_min_content_width(280);
    scroll.set_child(Some(&sidebar));
    scroll
}

fn main_panel() -> gtk::Box {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 18);
    panel.set_margin_top(28);
    panel.set_margin_bottom(28);
    panel.set_margin_start(28);
    panel.set_margin_end(28);
    panel.set_hexpand(true);
    panel.set_vexpand(true);

    let heading = gtk::Label::new(Some("YouTube client shell"));
    heading.set_xalign(0.0);
    heading.add_css_class("title-1");

    let summary = gtk::Label::new(Some(
        "Search, library, playback, and party controls will land here. The shell already keeps offline and room-mode guardrails visible while backend wiring grows.",
    ));
    summary.set_xalign(0.0);
    summary.set_wrap(true);

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search YouTube after OAuth is configured"));
    search.set_sensitive(false);

    panel.append(&heading);
    panel.append(&summary);
    panel.append(&search);
    panel.append(&playback_card());
    panel.append(&party_card());
    panel.append(&status_card("Offline policy", offline_policy_text()));

    panel
}

fn playback_card() -> gtk::Frame {
    let playback = Rc::new(RefCell::new(PlaybackState::default()));

    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.set_margin_top(14);
    card.set_margin_bottom(14);
    card.set_margin_start(14);
    card.set_margin_end(14);

    let title = gtk::Label::new(Some("Playback"));
    title.set_xalign(0.0);
    title.add_css_class("heading");

    let status = gtk::Label::new(Some(&playback_status_text(&playback.borrow())));
    status.set_xalign(0.0);

    let queue = gtk::Label::new(Some(&playback_queue_text(&playback.borrow())));
    queue.set_xalign(0.0);
    queue.add_css_class("dim-label");

    let current = gtk::Label::new(Some(&playback_current_text(&playback.borrow())));
    current.set_xalign(0.0);
    current.add_css_class("dim-label");

    let event = gtk::Label::new(Some(
        "Ready; queue is empty until search/import wiring lands.",
    ));
    event.set_xalign(0.0);
    event.set_wrap(true);
    event.add_css_class("dim-label");

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let previous = gtk::Button::with_label("Previous");
    let play_pause = gtk::Button::with_label("Play/Pause");
    let stop = gtk::Button::with_label("Stop");
    let next = gtk::Button::with_label("Next");

    controls.append(&previous);
    controls.append(&play_pause);
    controls.append(&stop);
    controls.append(&next);

    connect_playback_button(
        &play_pause,
        Rc::clone(&playback),
        &status,
        &queue,
        &current,
        &event,
        toggle_playback,
    );
    connect_playback_button(
        &stop,
        Rc::clone(&playback),
        &status,
        &queue,
        &current,
        &event,
        stop_playback,
    );
    connect_playback_button(
        &previous,
        Rc::clone(&playback),
        &status,
        &queue,
        &current,
        &event,
        skip_previous,
    );
    connect_playback_button(
        &next,
        Rc::clone(&playback),
        &status,
        &queue,
        &current,
        &event,
        skip_next,
    );

    card.append(&title);
    card.append(&status);
    card.append(&queue);
    card.append(&current);
    card.append(&event);
    card.append(&controls);

    let frame = gtk::Frame::new(None);
    frame.set_child(Some(&card));
    frame.add_css_class("card");
    frame
}

fn role_markup(role: PartyRole, label: &str) -> String {
    let color = match role {
        PartyRole::Admin => "red",
        PartyRole::Moderator => "orange",
        PartyRole::Client => "white",
        PartyRole::Guest => "gray",
    };
    let bold = matches!(role, PartyRole::Admin | PartyRole::Moderator);
    if bold {
        format!("<span foreground='{color}'><b>{label}</b></span>")
    } else {
        format!("<span foreground='{color}'>{label}</span>")
    }
}

fn party_card() -> gtk::Frame {
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

    let server = Rc::new(RefCell::new(server));

    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.set_margin_top(14);
    card.set_margin_bottom(14);
    card.set_margin_start(14);
    card.set_margin_end(14);

    let title_label = gtk::Label::new(Some("Party Room"));
    title_label.set_xalign(0.0);
    title_label.add_css_class("heading");

    let snap = server.borrow().snapshot();
    let state_label = gtk::Label::new(Some(&party_state_text(&snap)));
    state_label.set_xalign(0.0);

    let members_label = gtk::Label::new(None);
    members_label.set_xalign(0.0);
    members_label.set_use_markup(true);
    members_label.add_css_class("dim-label");

    let pending_label = gtk::Label::new(None);
    pending_label.set_xalign(0.0);
    pending_label.set_use_markup(true);
    pending_label.add_css_class("dim-label");

    let queue_label = gtk::Label::new(Some(&party_queue_text(&snap)));
    queue_label.set_xalign(0.0);
    queue_label.add_css_class("dim-label");

    let playback_label = gtk::Label::new(Some(&party_playback_text(&snap)));
    playback_label.set_xalign(0.0);
    playback_label.add_css_class("dim-label");

    let event_label = gtk::Label::new(Some("Demo room loaded; admin actions available."));
    event_label.set_xalign(0.0);
    event_label.set_wrap(true);
    event_label.add_css_class("dim-label");

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let lock_btn = gtk::Button::with_label("Lock");
    let unlock_btn = gtk::Button::with_label("Unlock");
    let end_btn = gtk::Button::with_label("End Room");
    controls.append(&lock_btn);
    controls.append(&unlock_btn);
    controls.append(&end_btn);

    update_members_markup(&snap, &members_label);
    update_pending_markup(&snap, &pending_label);

    let labels = PartyCardLabels {
        state: state_label.clone(),
        members: members_label.clone(),
        pending: pending_label.clone(),
        queue: queue_label.clone(),
        playback: playback_label.clone(),
        event: event_label.clone(),
    };

    connect_party_button(
        &lock_btn,
        Rc::clone(&server),
        labels.clone(),
        |srv| match srv.lock_room("admin-1") {
            Ok(()) => "Room locked.".to_string(),
            Err(e) => format!("Lock failed: {e}"),
        },
    );
    connect_party_button(
        &unlock_btn,
        Rc::clone(&server),
        labels.clone(),
        |srv| match srv.unlock_room("admin-1") {
            Ok(()) => "Room unlocked.".to_string(),
            Err(e) => format!("Unlock failed: {e}"),
        },
    );
    connect_party_button(
        &end_btn,
        Rc::clone(&server),
        labels.clone(),
        |srv| match srv.end_room("admin-1") {
            Ok(()) => "Room ended.".to_string(),
            Err(e) => format!("End failed: {e}"),
        },
    );

    let join_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let approve_btn = gtk::Button::with_label("Approve Next");
    let reject_btn = gtk::Button::with_label("Reject Next");
    join_controls.append(&approve_btn);
    join_controls.append(&reject_btn);

    connect_party_button(&approve_btn, Rc::clone(&server), labels.clone(), |srv| {
        let ids: Vec<String> = srv.join_requests().map(|r| r.request_id.clone()).collect();
        if ids.is_empty() {
            return "no pending requests to approve".to_string();
        }
        match srv.approve_join("admin-1", &ids[0], PartyRole::Client, 9999) {
            Ok(_) => format!("approved request: {}", ids[0]),
            Err(e) => format!("approve failed: {e}"),
        }
    });
    connect_party_button(&reject_btn, Rc::clone(&server), labels.clone(), |srv| {
        let ids: Vec<String> = srv.join_requests().map(|r| r.request_id.clone()).collect();
        if ids.is_empty() {
            return "no pending requests to reject".to_string();
        }
        match srv.reject_join("admin-1", &ids[0]) {
            Ok(_) => format!("rejected request: {}", ids[0]),
            Err(e) => format!("reject failed: {e}"),
        }
    });

    let discovery = Rc::new(RefCell::new(None::<LanDiscoveryHandle>));
    let discovery_label = gtk::Label::new(Some("LAN discovery off"));
    discovery_label.set_xalign(0.0);
    discovery_label.add_css_class("dim-label");

    let discovery_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let start_btn = gtk::Button::with_label("Start Discovery");
    let stop_btn = gtk::Button::with_label("Stop Discovery");
    stop_btn.set_sensitive(false);
    discovery_controls.append(&start_btn);
    discovery_controls.append(&stop_btn);

    let dl1 = discovery_label.clone();
    let start1 = start_btn.clone();
    let stop1 = stop_btn.clone();
    let disc1 = Rc::clone(&discovery);
    start_btn.connect_clicked(move |_| {
        let handle = LanDiscoveryHandle::start();
        if handle.is_some() {
            *disc1.borrow_mut() = handle;
            dl1.set_text("LAN discovery running...");
            start1.set_sensitive(false);
            stop1.set_sensitive(true);
        } else {
            dl1.set_text("LAN discovery failed to start");
        }
    });

    let dl2 = discovery_label.clone();
    let start2 = start_btn.clone();
    let stop2 = stop_btn.clone();
    let disc2 = Rc::clone(&discovery);
    stop_btn.connect_clicked(move |_| {
        *disc2.borrow_mut() = None;
        dl2.set_text("LAN discovery stopped");
        start2.set_sensitive(true);
        stop2.set_sensitive(false);
    });

    let disc3 = Rc::clone(&discovery);
    let dl3 = discovery_label.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
        let guard = disc3.borrow();
        if let Some(handle) = guard.as_ref() {
            let rooms = handle.discovered_rooms();
            if rooms.is_empty() {
                dl3.set_text("LAN discovery running — no rooms found yet");
            } else {
                let names: Vec<&str> = rooms.iter().map(|r| r.room_name.as_str()).collect();
                dl3.set_text(&format!(
                    "Discovered ({}): {}",
                    rooms.len(),
                    names.join(", ")
                ));
            }
        }
        glib::ControlFlow::Continue
    });

    card.append(&title_label);
    card.append(&state_label);
    card.append(&members_label);
    card.append(&pending_label);
    card.append(&join_controls);
    card.append(&discovery_label);
    card.append(&discovery_controls);
    card.append(&queue_label);
    card.append(&playback_label);
    card.append(&event_label);
    card.append(&controls);

    let frame = gtk::Frame::new(None);
    frame.set_child(Some(&card));
    frame.add_css_class("card");
    frame
}

fn members_markup(snap: &RoomSnapshot) -> String {
    let mut members = snap.members.clone();
    members.sort_by(|a, b| a.client_id.cmp(&b.client_id));
    let header = format!("Members ({}):", members.len());
    let mut parts = vec![header];
    for m in &members {
        let role_label = format!("{:?}", m.role);
        let colored = role_markup(m.role, &role_label);
        parts.push(format!("  {colored} | {} | {}", m.user_name, m.client_id));
    }
    parts.join("\n")
}

fn pending_markup(snap: &RoomSnapshot) -> Option<String> {
    let pending = &snap.pending_requests;
    if pending.is_empty() {
        return None;
    }
    let header = format!("Pending requests ({}):", pending.len());
    let mut parts = vec![header];
    for req in pending {
        let code = req.invite_code_attempt.as_deref().unwrap_or("no code");
        parts.push(format!(
            "  <span foreground='cyan'>{}</span> ({}) — {}",
            req.user_name, req.device_name, code
        ));
    }
    Some(parts.join("\n"))
}

fn update_members_markup(snap: &RoomSnapshot, label: &gtk::Label) {
    label.set_markup(&members_markup(snap));
}

fn update_pending_markup(snap: &RoomSnapshot, label: &gtk::Label) {
    match pending_markup(snap) {
        Some(markup) => {
            label.set_visible(true);
            label.set_markup(&markup);
        }
        None => label.set_visible(false),
    }
}

#[derive(Clone)]
struct PartyCardLabels {
    state: gtk::Label,
    members: gtk::Label,
    pending: gtk::Label,
    queue: gtk::Label,
    playback: gtk::Label,
    event: gtk::Label,
}

fn connect_party_button(
    button: &gtk::Button,
    server: Rc<RefCell<RoomServer>>,
    labels: PartyCardLabels,
    action: fn(&mut RoomServer) -> String,
) {
    button.connect_clicked(move |_| {
        let event_text = action(&mut server.borrow_mut());
        let snap = server.borrow().snapshot();
        labels.state.set_text(&party_state_text(&snap));
        update_members_markup(&snap, &labels.members);
        update_pending_markup(&snap, &labels.pending);
        labels.queue.set_text(&party_queue_text(&snap));
        labels.playback.set_text(&party_playback_text(&snap));
        labels.event.set_text(&event_text);
    });
}

fn party_state_text(snap: &RoomSnapshot) -> String {
    format!(
        "Room: {} | State: {:?} | Protocol v{}",
        snap.room.room_name, snap.room.state, snap.protocol_version
    )
}

fn party_queue_text(snap: &RoomSnapshot) -> String {
    let header = format!("Queue ({} item(s)):", snap.queue.len());
    if snap.queue.is_empty() {
        return format!("{header}\n  (empty)");
    }
    let rows: Vec<String> = snap
        .queue
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
                TrackRef::ImportedLocalFile { title, .. } => format!("[local] {title}"),
            };
            format!(
                "  {} | votes:{:+} | {}",
                item.queue_item_id, item.votes, title
            )
        })
        .collect();
    std::iter::once(header)
        .chain(rows)
        .collect::<Vec<_>>()
        .join("\n")
}

fn party_playback_text(snap: &RoomSnapshot) -> String {
    let pb = &snap.playback_state;
    match &pb.track_ref {
        Some(TrackRef::YouTube {
            title, video_id, ..
        }) => format!(
            "Playing: {} ({}) — {} ms",
            title.as_deref().unwrap_or("(no title)"),
            video_id,
            pb.position_ms
        ),
        Some(TrackRef::ImportedLocalFile { title, .. }) => {
            format!("Playing: [local] {title} — {} ms", pb.position_ms)
        }
        None => "Playback: idle".to_string(),
    }
}

fn connect_playback_button(
    button: &gtk::Button,
    playback: Rc<RefCell<PlaybackState>>,
    status: &gtk::Label,
    queue: &gtk::Label,
    current: &gtk::Label,
    event: &gtk::Label,
    action: fn(&mut PlaybackState) -> String,
) {
    let status = status.clone();
    let queue = queue.clone();
    let current = current.clone();
    let event = event.clone();

    button.connect_clicked(move |_| {
        let mut playback = playback.borrow_mut();
        let event_text = action(&mut playback);
        refresh_playback_labels(&playback, &status, &queue, &current);
        event.set_text(&event_text);
    });
}

fn refresh_playback_labels(
    playback: &PlaybackState,
    status: &gtk::Label,
    queue: &gtk::Label,
    current: &gtk::Label,
) {
    status.set_text(&playback_status_text(playback));
    queue.set_text(&playback_queue_text(playback));
    current.set_text(&playback_current_text(playback));
}

fn playback_status_text(playback: &PlaybackState) -> String {
    format!(
        "Playback: {} at {} ms",
        playback_status_name(playback.status),
        playback.position_ms
    )
}

fn playback_queue_text(playback: &PlaybackState) -> String {
    format!("Queue items: {}", playback.queue.len())
}

fn playback_current_text(playback: &PlaybackState) -> String {
    playback
        .current()
        .map(|item| format!("Current: {}", item.title))
        .unwrap_or_else(|| "Current: none".to_string())
}

fn playback_status_name(status: PlaybackStatus) -> &'static str {
    match status {
        PlaybackStatus::Stopped => "stopped",
        PlaybackStatus::Playing => "playing",
        PlaybackStatus::Paused => "paused",
    }
}

fn toggle_playback(playback: &mut PlaybackState) -> String {
    match playback.status {
        PlaybackStatus::Playing => {
            playback.pause();
            "Playback paused.".to_string()
        }
        PlaybackStatus::Stopped | PlaybackStatus::Paused => match playback.play() {
            Ok(()) => "Playback started.".to_string(),
            Err(PlaybackError::QueueEmpty) => {
                "Queue empty; add tracks after search/import wiring lands.".to_string()
            }
        },
    }
}

fn stop_playback(playback: &mut PlaybackState) -> String {
    playback.stop();
    "Playback stopped.".to_string()
}

fn skip_previous(playback: &mut PlaybackState) -> String {
    playback
        .skip_previous()
        .map(|item| format!("Skipped to previous: {}", item.title))
        .unwrap_or_else(|| "No previous item; playback stopped.".to_string())
}

fn skip_next(playback: &mut PlaybackState) -> String {
    playback
        .skip_next()
        .map(|item| format!("Skipped to next: {}", item.title))
        .unwrap_or_else(|| "No next item; playback stopped.".to_string())
}

fn status_card(title: &str, body: &str) -> gtk::Frame {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.set_margin_top(14);
    card.set_margin_bottom(14);
    card.set_margin_start(14);
    card.set_margin_end(14);

    let title = gtk::Label::new(Some(title));
    title.set_xalign(0.0);
    title.add_css_class("heading");

    let body = gtk::Label::new(Some(body));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.add_css_class("dim-label");

    card.append(&title);
    card.append(&body);

    let frame = gtk::Frame::new(None);
    frame.set_child(Some(&card));
    frame.add_css_class("card");
    frame
}

fn shell_sections() -> [ShellSection; 5] {
    [
        ShellSection {
            title: "Home",
            detail: "Status and setup checklist",
        },
        ShellSection {
            title: "Search",
            detail: "YouTube Data API v3 search only",
        },
        ShellSection {
            title: "Library",
            detail: "Local playlists, follows, favorites",
        },
        ShellSection {
            title: "Party",
            detail: "LAN room controls and permissions",
        },
        ShellSection {
            title: "Offline",
            detail: "Local imports, no YouTube audio persistence",
        },
    ]
}

fn offline_policy_text() -> &'static str {
    if can_persist_youtube_audio() {
        "YouTube audio persistence is enabled. Verify explicit rights before use."
    } else {
        "YouTube audio persistence is disabled. Offline mode uses local files and metadata references."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meowify_party::{PartyPermission, can};

    #[test]
    fn shell_sections_include_party_and_offline_guardrails() {
        let sections = shell_sections();

        assert!(sections.iter().any(|section| section.title == "Party"));
        assert!(sections.iter().any(|section| section.title == "Offline"));
    }

    #[test]
    fn gtk_copy_reflects_core_offline_policy() {
        assert!(!can_persist_youtube_audio());
        assert!(offline_policy_text().contains("disabled"));
    }

    #[test]
    fn gtk_copy_reflects_party_rbac_policy() {
        assert!(!can(PartyRole::Client, PartyPermission::ControlPlayback));
    }

    #[test]
    fn party_state_text_includes_room_name_state_and_protocol() {
        use meowify_party::{
            ConnectionState, PartyClient, PartyRole, RoomServer, RoomSnapshot, RoomVisibility,
        };
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
            "Test Room",
            RoomVisibility::LanVisible,
            admin,
            "invite",
            0,
        );
        let snap: RoomSnapshot = server.snapshot();

        let text = party_state_text(&snap);
        assert!(text.contains("Test Room"));
        assert!(text.contains("WaitingForClients"));
        assert!(text.contains("Protocol v"));
    }

    #[test]
    fn party_playback_text_reports_idle_without_track() {
        use meowify_party::{ConnectionState, PartyClient, PartyRole, RoomServer, RoomVisibility};
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
            "Test Room",
            RoomVisibility::LanVisible,
            admin,
            "invite",
            0,
        );
        let snap = server.snapshot();

        assert_eq!(party_playback_text(&snap), "Playback: idle");
    }

    #[test]
    fn members_markup_lists_each_member() {
        use meowify_party::{
            ConnectionState, JoinRequest, PartyClient, PartyRole, RoomServer, RoomVisibility,
        };
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
        let mut server =
            RoomServer::create("r1", "Room", RoomVisibility::LanVisible, admin, "inv", 0);
        server
            .handle_join_request(JoinRequest {
                request_id: "req-bob".to_string(),
                room_id: "r1".to_string(),
                client_id: "client-bob".to_string(),
                user_name: "Bob".to_string(),
                device_name: "phone".to_string(),
                invite_code_attempt: None,
                requested_at_ms: 100,
            })
            .unwrap();
        server
            .approve_join("admin-1", "req-bob", PartyRole::Client, 200)
            .unwrap();
        let snap = server.snapshot();

        let markup = members_markup(&snap);
        assert!(markup.contains("Members (2)"));
        assert!(markup.contains("Alice"));
        assert!(markup.contains("Bob"));
    }

    #[test]
    fn party_queue_text_lists_track_titles() {
        use meowify_party::{
            ConnectionState, PartyClient, PartyRole, RoomServer, RoomVisibility, TrackRef,
        };
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
        let mut server =
            RoomServer::create("r1", "Room", RoomVisibility::LanVisible, admin, "inv", 0);
        server
            .add_queue_item(
                "admin-1",
                "item-1",
                TrackRef::YouTube {
                    video_id: "abc".to_string(),
                    title: Some("Test Song".to_string()),
                    channel_title: Some("Test Artist".to_string()),
                },
            )
            .unwrap();
        let snap = server.snapshot();

        let text = party_queue_text(&snap);
        assert!(text.contains("Queue (1 item(s)):"));
        assert!(text.contains("Test Song"));
        assert!(text.contains("Test Artist"));
        assert!(text.contains("votes:+0"));
    }

    #[test]
    fn gtk_playback_copy_reflects_empty_default_state() {
        let playback = PlaybackState::default();

        assert_eq!(playback_status_text(&playback), "Playback: stopped at 0 ms");
        assert_eq!(playback_queue_text(&playback), "Queue items: 0");
        assert_eq!(playback_current_text(&playback), "Current: none");
    }

    #[test]
    fn gtk_playback_play_reports_empty_queue() {
        let mut playback = PlaybackState::default();

        let event = toggle_playback(&mut playback);

        assert_eq!(playback.status, PlaybackStatus::Stopped);
        assert!(event.contains("Queue empty"));
    }

    #[test]
    fn gtk_playback_stop_resets_position() {
        let mut playback = PlaybackState {
            position_ms: 90_000,
            ..PlaybackState::default()
        };

        let event = stop_playback(&mut playback);

        assert_eq!(playback_status_text(&playback), "Playback: stopped at 0 ms");
        assert_eq!(event, "Playback stopped.");
    }

    #[test]
    fn role_markup_admin_uses_red_bold() {
        let markup = role_markup(PartyRole::Admin, "Admin");
        assert!(markup.contains("foreground='red'"));
        assert!(markup.contains("<b>Admin</b>"));
    }

    #[test]
    fn role_markup_client_uses_white() {
        let markup = role_markup(PartyRole::Client, "Client");
        assert!(markup.contains("foreground='white'"));
        assert!(!markup.contains("<b>"));
    }

    #[test]
    fn members_markup_includes_role_colors() {
        use meowify_party::{
            ConnectionState, JoinRequest, PartyClient, PartyRole, RoomServer, RoomVisibility,
        };
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
        let mut server =
            RoomServer::create("r1", "Room", RoomVisibility::LanVisible, admin, "inv", 0);
        server
            .handle_join_request(JoinRequest {
                request_id: "req-bob".to_string(),
                room_id: "r1".to_string(),
                client_id: "client-bob".to_string(),
                user_name: "Bob".to_string(),
                device_name: "phone".to_string(),
                invite_code_attempt: None,
                requested_at_ms: 100,
            })
            .unwrap();
        server
            .approve_join("admin-1", "req-bob", PartyRole::Client, 200)
            .unwrap();
        let snap = server.snapshot();

        let markup = members_markup(&snap);
        assert!(markup.contains("Members (2)"));
        assert!(markup.contains("Alice"));
        assert!(markup.contains("Bob"));
        assert!(markup.contains("foreground='red'"));
    }

    #[test]
    fn pending_markup_returns_some_with_requests() {
        use meowify_party::{
            ConnectionState, JoinRequest, PartyClient, PartyRole, RoomServer, RoomVisibility,
        };
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
        let mut server =
            RoomServer::create("r1", "Room", RoomVisibility::LanVisible, admin, "inv", 0);
        server
            .handle_join_request(JoinRequest {
                request_id: "req-carol".to_string(),
                room_id: "r1".to_string(),
                client_id: "client-carol".to_string(),
                user_name: "Carol".to_string(),
                device_name: "tablet".to_string(),
                invite_code_attempt: None,
                requested_at_ms: 300,
            })
            .unwrap();
        let snap = server.snapshot();

        let markup = pending_markup(&snap);
        assert!(markup.is_some());
        let text = markup.unwrap();
        assert!(text.contains("Pending requests (1)"));
        assert!(text.contains("Carol"));
    }

    #[test]
    fn pending_markup_returns_none_when_empty() {
        use meowify_party::{ConnectionState, PartyClient, PartyRole, RoomServer, RoomVisibility};
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
        let server = RoomServer::create("r1", "Room", RoomVisibility::LanVisible, admin, "inv", 0);
        let snap = server.snapshot();

        assert!(pending_markup(&snap).is_none());
    }
}
