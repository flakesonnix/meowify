# SoundCloud API Notes

## Auth
- Use OAuth 2.1 Authorization Code + PKCE for user resources.
- Use Client Credentials only for public-resource access.
- Store tokens in system keyring where available.
- Refresh tokens are single-use; update stored refresh token atomically.
- Send `Authorization: OAuth ACCESS_TOKEN`.

## Playback
- Use only documented stream endpoints.
- Treat `access=blocked` as metadata-only.
- Treat `access=preview` as preview-only.
- Do not scrape web player internals.
- Do not share raw audio in party mode.

## Downloads And Offline
- API fields `downloadable` and `download_url` exist.
- API Terms prohibit persistent file-save/offline storage of SoundCloud User Content by apps.
- Implement compliant alternatives first:
  - metadata-only offline references
  - user-owned imported local files
  - open in browser
  - unavailable offline state
  - local playlists containing references

## Search/Browse
- Tracks: `GET /tracks`.
- Users: `GET /users`.
- Playlists: `GET /playlists`.
- Resolve SoundCloud URLs with `GET /resolve`.
- Prefer `linked_partitioning=true`.
