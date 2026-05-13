# Research Summary

Research date: 2026-05-13.

## Official Docs Used

### YouTube
- YouTube Data API v3 docs: https://developers.google.com/youtube/v3/docs
- YouTube API Terms: https://developers.google.com/youtube/terms/api-services-terms-of-service
- Google OAuth2 PKCE: https://developers.google.com/identity/protocols/oauth2/pkce

### SoundCloud
- SoundCloud API Guide: https://developers.soundcloud.com/docs/api/guide
- SoundCloud API Reference: https://developers.soundcloud.com/docs/api/reference
- SoundCloud API Terms: https://developers.soundcloud.com/docs/api/terms-of-use
- SoundCloud Rate Limits: https://developers.soundcloud.com/docs/api/rate-limits

## Legal Constraints (both platforms)
- API access requires registered app credentials.
- User Content belongs to uploaders/rightsholders.
- Apps must not scrape, rip, capture, copy, or bypass content restrictions.
- No persistent file-save/offline access to platform content.
- User actions must be explicitly initiated by the authenticated user.
- Attribution required when displaying/streaming content.

## Chosen Crates
- `tokio`: async runtime
- `reqwest`: HTTP client with Rustls
- `serde`/`serde_json`: JSON serialization
- `thiserror`/`anyhow`: error handling
- `tracing`/`tracing-subscriber`: logging
- `clap`: CLI argument parsing
- `directories`: XDG paths
- `rusqlite`: embedded SQLite
- `gtk4`/`libadwaita`: GNOME desktop UI
- `ratatui`/`crossterm`: terminal UI
- `gstreamer-player`: audio playback
- `keyring`: secret storage
- `libp2p`: mDNS LAN discovery + request/response
- `bluer`: Linux Bluetooth experiment
- `mpris-server`: MPRIS D-Bus integration
- `sha2`: hashing (invite codes)

## Risks
- Desktop secret handling is fragile for OAuth.
- Stream URLs require authentication and may be restricted.
- Download/offline caching conflicts with API terms.
- Offline mode must rely on local imports and metadata refs.
