use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use meowify_core::can_persist_youtube_audio;
use meowify_party::{PartyPermission, PartyRole, can};
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppState {
    selected: usize,
    should_quit: bool,
    playback: PlaybackState,
    last_event: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            selected: 0,
            should_quit: false,
            playback: PlaybackState::default(),
            last_event: "ready; queue is empty until search/import wiring lands".to_string(),
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
            _ => {}
        }
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
    let party_policy = if can(PartyRole::Client, PartyPermission::ControlPlayback) {
        "Client playback control: allowed by default"
    } else {
        "Client playback control: denied by default; enforce RBAC in handlers"
    };

    Paragraph::new(vec![
        Line::from(Span::styled(
            selected.title(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(selected.detail()),
        Line::from(""),
        Line::from("Keys: j/down next view, k/up previous view, space play/pause, s stop, n/p skip, q/esc quit"),
        Line::from(""),
        Line::from(playback_status_line(&app.playback)),
        Line::from(playback_queue_line(&app.playback)),
        Line::from(playback_current_line(&app.playback)),
        Line::from(format!("Last event: {}", app.last_event)),
        Line::from(""),
        Line::from(offline_policy),
        Line::from(party_policy),
    ])
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
