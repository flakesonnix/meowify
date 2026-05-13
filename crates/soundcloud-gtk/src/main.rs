use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::glib;
use meowify_core::can_persist_youtube_audio;
use meowify_party::{PartyPermission, PartyRole, can};
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
    panel.append(&status_card("Offline policy", offline_policy_text()));
    panel.append(&status_card("Party mode", party_policy_text()));

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

fn party_policy_text() -> &'static str {
    if can(PartyRole::Client, PartyPermission::ControlPlayback) {
        "Clients can control playback by default. Recheck role policy before networking lands."
    } else {
        "Normal clients cannot control playback by default; protocol handlers must enforce RBAC."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(party_policy_text().contains("cannot control playback"));
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
}
