# Room Protocol Notes

## Goals
- LAN-first local party mode.
- One active admin per room.
- Admin is source of truth for playback state and queue sequence numbers.
- Clients receive roles and session tokens after approval.
- SoundCloud audio is never redistributed; only refs and commands are synced.

## Transport Plan
- Discovery: libp2p mDNS.
- Control messages: libp2p request/response with serde JSON first.
- Bluetooth: bluer experiment for discovery/pairing/control only.

## Security Rules
- Invite codes are random, short-lived, and stored only as hashes.
- Every state-changing message carries room ID, client ID, session token, monotonic sequence number, and timestamp.
- Protocol handlers call `require_permission` before state changes.
- Rate-limit join requests, queue requests, votes, and chat.
- Network features are opt-in and LAN-only by default.

## Rights Rules
- Sync SoundCloud track refs and playback commands only.
- Each client streams through its own authorized SoundCloud session where required.
- If a client cannot access a track, show unavailable/open in browser/skip locally.
- Imported local files are never transferred by default.
