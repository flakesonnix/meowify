# API Notes

## YouTube Data API v3

- Base URL: `https://www.googleapis.com/youtube/v3`
- Auth: Google OAuth2 with PKCE, scope `https://www.googleapis.com/auth/youtube.readonly`
- Pagination: `pageToken` / `nextPageToken`, `maxResults` max 50
- Search: `GET /search?part=snippet&type=video&q=...`
- Video detail: `GET /videos?part=snippet,contentDetails,status&id=...`
- Channel detail: `GET /channels?part=snippet&id=...`
- Playlist detail: `GET /playlists?part=snippet&id=...`
- Playlist items: `GET /playlistItems?part=snippet,contentDetails&playlistId=...`
- Channel playlists: `GET /playlists?part=snippet&channelId=...&maxResults=50`

## SoundCloud API

- Base URL: `https://api.soundcloud.com`
- Auth: OAuth 2.1 with PKCE (user) or Client Credentials (public)
- Header: `Authorization: OAuth ACCESS_TOKEN`
- Pagination: `linked_partitioning=true`, `limit` max 200, `next_href`
- Search tracks: `GET /tracks?q=...`
- Search users: `GET /users?q=...`
- Search playlists: `GET /playlists?q=...`
- Track detail: `GET /tracks/{track_urn}`
- Track streams: `GET /tracks/{track_urn}/streams`
- Track access: `playable`, `preview`, `blocked`
- Playlist CRUD: `/playlists` and `/playlists/{playlist_urn}`
- Follows: `/me/followings`
- Likes: `/likes/tracks/{track_urn}` and `/likes/playlists/{playlist_urn}`
- URL resolve: `GET /resolve?url=...`

## Shared Rules

- Use only documented endpoints for both platforms.
- Treat restricted content as metadata-only.
- Each client streams through its own authorized session.
- Do not share raw audio in party mode.
- Download/cache only when terms explicitly allow.
