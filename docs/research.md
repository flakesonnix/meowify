# Research Summary

Research date: 2026-05-13.

## Official SoundCloud Docs Used
- SoundCloud API Guide: https://developers.soundcloud.com/docs/api/guide
- SoundCloud API Reference: https://developers.soundcloud.com/docs/api/reference
- SoundCloud OpenAPI/API Explorer JSON: https://developers.soundcloud.com/docs/api/explorer/api.json
- SoundCloud API Terms: https://developers.soundcloud.com/docs/api/terms-of-use
- SoundCloud Rate Limits: https://developers.soundcloud.com/docs/api/rate-limits

## SoundCloud API Terms And Legal Constraints
- API access requires registered app credentials.
- Security codes, client secrets, and tokens must be protected.
- User Content belongs to uploaders/rightsholders.
- Apps must not scrape SoundCloud.
- Apps must not rip, capture, copy, or bypass content restrictions.
- Apps must not provide persistent file-save/offline access to SoundCloud User Content. Terms allow only session-based caching required for operation, and it must cease to be playable after the session.
- User actions such as playback, likes, follows, comments, uploads, reposts, playlist edits, and remote mutations must be explicitly initiated by the authenticated user.
- Attribution is required when displaying/streaming User Content: uploader, SoundCloud source, and permalink.

## Confirmed SoundCloud Features
- Base API URL: `https://api.soundcloud.com`.
- Auth URL: `https://secure.soundcloud.com`.
- OAuth 2.1 with PKCE is documented.
- Authorization Code flow supports user resources.
- Client Credentials flow supports public resources.
- Access tokens expire around one hour.
- Refresh tokens are single-use.
- API requests use `Authorization: OAuth ACCESS_TOKEN`.
- Search tracks: `GET /tracks`.
- Search users: `GET /users`.
- Search playlists: `GET /playlists`.
- Track details: `GET /tracks/{track_urn}`.
- Track streams: `GET /tracks/{track_urn}/streams`.
- Preview endpoint: `GET /tracks/{track_urn}/preview`.
- Playlist details and CRUD: `/playlists` and `/playlists/{playlist_urn}`.
- Playlist tracks: `GET /playlists/{playlist_urn}/tracks`.
- Authenticated user playlists: `GET /me/playlists`.
- Follows: `/me/followings` endpoints.
- Likes: `/likes/tracks/{track_urn}` and `/likes/playlists/{playlist_urn}` endpoints.
- URL resolve: `GET /resolve`.
- Track `access` values: `playable`, `preview`, `blocked`.
- Track fields include `downloadable`, `download_url`, `streamable`, deprecated `stream_url`.
- Pagination uses `linked_partitioning=true`, `limit` max 200, `next_href`.
- Play stream requests are limited to 15,000 per 24-hour window per client ID.

## Uncertain SoundCloud Features
- Current app registration availability and review constraints.
- Exact scopes returned for a newly registered app.
- Desktop custom-scheme redirect behavior in production.
- Runtime shape of stream responses for different regions/content states.
- Whether `download_url` may be used by third-party apps without separate written permission, given API Terms file-save prohibition.
- Commercial use approval and product positioning risk for a full desktop client.

## Chosen Crates
- `tokio`: async runtime.
- `reqwest`: HTTP client with Rustls and JSON support.
- `serde`/`serde_json`: API/protocol JSON.
- `thiserror`/`anyhow`: library and app errors.
- `tracing`/`tracing-subscriber`: structured logs.
- `clap`: CLI flags and party commands.
- `directories`: XDG config/cache/data paths.
- `rusqlite`: embedded SQLite, simple local DB ownership model.
- `gtk4`/`libadwaita`: GNOME desktop UI.
- `ratatui`/`crossterm`: terminal UI.
- `gstreamer-player`: Linux-friendly streaming/local playback backend.
- `keyring`: Secret Service/native token storage.
- `libp2p`: mDNS LAN discovery and request/response protocol.
- `bluer`: Linux BlueZ Bluetooth experiment.

## API Explorer Checks Still Required
- Verify all target endpoints with real app credentials before shipping.
- Confirm required request/response fields for playlist create/update.
- Confirm stream endpoint output for playable, preview, blocked, private, and geo-blocked tracks.
- Confirm error bodies for 401, 403, 404, 429, and 503.
- Confirm current behavior of `downloadable` and `download_url` without implementing persistent downloads.

## Risks
- Authentication requires confidential client credentials; desktop secret handling is fragile.
- Client Credentials tokens are rate-limited and should be reused/refreshed.
- Stream URLs require authentication and may be restricted by uploader, region, paywall, or private access.
- Download/offline SoundCloud audio conflicts with API Terms unless SoundCloud grants separate permission or applicable law/user agreement clearly permits it.
- Offline mode must avoid network requests and must not play persistent cached SoundCloud User Content.
