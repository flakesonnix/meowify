# Meowify

Multi-platform media client supporting YouTube and SoundCloud, written in Rust.
Works fully offline/local-first with LAN party mode — no account required for local features.

## Interfaces
- GTK4/libadwaita desktop app with GStreamer audio.
- Ratatui/Crossterm terminal app with GStreamer audio.
- Debug/admin CLI with snapshot output.

## Crates
- `soundcloud-core`: YouTube Data API v3 + SoundCloud API client, OAuth2, config, SQLite, local library, playlists, follows/favorites, cache.
- `soundcloud-playback`: playback abstraction, GStreamer backend, queue, repeat/shuffle, volume.
- `soundcloud-party`: LAN room/party mode, libp2p mDNS discovery, protocol, RBAC, playback sync.
- `soundcloud-gtk`: GTK4/libadwaita frontend with party controls and file import.
- `soundcloud-tui`: Ratatui frontend with party controls, file import, and progress bar.
- `soundcloud-cli`: debugging/admin CLI with snapshot filters.

## Legal And API Rules
- No scraping or stream ripping for either platform.
- No bypassing playback/download restrictions.
- No redistributing protected audio in party mode.
- Offline mode uses local imports, metadata references, and marked-unavailable placeholders.
- User actions (likes, follows, comments, etc.) must be explicitly initiated by the authenticated user.

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

## API Credentials
Register an app with YouTube and/or SoundCloud as needed. Provide credentials through environment variables or settings. Do not commit client secrets or tokens. The app works fully offline without any credentials.
