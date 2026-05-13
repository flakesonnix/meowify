# Meowify

Meowify is an experimental SoundCloud desktop and terminal client written primarily in Rust.

It is not an official SoundCloud app. It must use documented SoundCloud APIs only and must respect SoundCloud API Terms, uploader rights, authentication requirements, and playback/download restrictions.

## Planned Interfaces
- GTK4/libadwaita desktop app.
- Ratatui/Crossterm terminal app.
- Debug/admin CLI.

## Planned Crates
- `soundcloud-core`: API, auth, config, SQLite, local library, playlists, local follows/favorites, metadata cache.
- `soundcloud-playback`: playback abstraction, queue, repeat/shuffle, GStreamer backend.
- `soundcloud-party`: LAN-only room mode, protocol, roles, permissions, playback sync.
- `soundcloud-gtk`: GTK4/libadwaita frontend.
- `soundcloud-tui`: Ratatui frontend.
- `soundcloud-cli`: debugging/admin CLI.

## Legal And API Rules
- No scraping.
- No stream ripping.
- No bypassing playback/download restrictions.
- No redistributing SoundCloud audio.
- No persistent SoundCloud audio downloads or offline cache unless SoundCloud terms and rights explicitly allow it.
- Offline mode uses local imports, local metadata, and SoundCloud references marked unavailable when needed.
- User actions such as likes, follows, comments, playlist edits, uploads, and playback must be explicitly initiated by the authenticated user.

## Development
```sh
nix develop
cargo build
cargo test
cargo nextest run
cargo clippy
cargo fmt
just test
just run-gtk
just run-tui
```

## SoundCloud Credentials
Register an app with SoundCloud and provide credentials through environment variables or settings. Do not commit client secrets or tokens.
