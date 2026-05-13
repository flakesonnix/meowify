use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use libp2p::{
    Multiaddr, PeerId, StreamProtocol,
    futures::StreamExt as _,
    mdns, noise,
    request_response::{self, ProtocolSupport, json as rr_json},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::model::{RoomId, RoomVisibility};
use crate::protocol::PROTOCOL_VERSION;

const ANNOUNCE_PROTOCOL: &str = "/meowify/announce/1";

/// Room metadata broadcast to peers discovered on the LAN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomAnnouncement {
    pub room_id: RoomId,
    pub room_name: String,
    pub visibility: RoomVisibility,
    pub admin_name: String,
    pub protocol_version: u32,
}

impl RoomAnnouncement {
    pub fn new(
        room_id: impl Into<RoomId>,
        room_name: impl Into<String>,
        visibility: RoomVisibility,
        admin_name: impl Into<String>,
    ) -> Self {
        Self {
            room_id: room_id.into(),
            room_name: room_name.into(),
            visibility,
            admin_name: admin_name.into(),
            protocol_version: PROTOCOL_VERSION,
        }
    }
}

/// Events emitted by [`LanDiscovery`] as peers appear/disappear or announce rooms.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    PeerExpired {
        peer_id: PeerId,
    },
    RoomAnnounced {
        peer_id: PeerId,
        announcement: RoomAnnouncement,
    },
    RoomExpired {
        peer_id: PeerId,
        room_id: RoomId,
    },
    /// A peer sent an announcement with a protocol version that differs from ours.
    /// The room is not stored; the peer should be treated as incompatible.
    ProtocolVersionMismatch {
        peer_id: PeerId,
        their_version: u32,
        our_version: u32,
    },
}

// ── NetworkBehaviour ──────────────────────────────────────────────────────────
//
// Request = RoomAnnouncement (initiator's room, mandatory)
// Response = Option<RoomAnnouncement> (responder's room, optional)

type AnnounceEvent = request_response::Event<RoomAnnouncement, Option<RoomAnnouncement>>;

#[derive(NetworkBehaviour)]
struct DiscoveryBehaviour {
    mdns: mdns::tokio::Behaviour,
    announce: rr_json::Behaviour<RoomAnnouncement, Option<RoomAnnouncement>>,
}

// ── LanDiscovery ─────────────────────────────────────────────────────────────

/// mDNS-based LAN peer discovery with JSON room announcement exchange.
///
/// On mDNS peer discovery, sends the local `RoomAnnouncement` (if set) as a
/// request-response request. Handles inbound requests by replying with the
/// local announcement. Emits `DiscoveryEvent`s over the provided channel.
pub struct LanDiscovery {
    swarm: libp2p::Swarm<DiscoveryBehaviour>,
    peers: HashMap<PeerId, Vec<Multiaddr>>,
    known_rooms: HashMap<PeerId, RoomAnnouncement>,
    local_announcement: Option<RoomAnnouncement>,
}

impl LanDiscovery {
    pub fn new(
        local_announcement: Option<RoomAnnouncement>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                Ok(DiscoveryBehaviour {
                    mdns: mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        key.public().to_peer_id(),
                    )?,
                    announce: rr_json::Behaviour::new(
                        [(
                            StreamProtocol::new(ANNOUNCE_PROTOCOL),
                            ProtocolSupport::Full,
                        )],
                        request_response::Config::default(),
                    ),
                })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(Self {
            swarm,
            peers: HashMap::new(),
            known_rooms: HashMap::new(),
            local_announcement,
        })
    }

    pub fn local_peer_id(&self) -> &PeerId {
        self.swarm.local_peer_id()
    }

    pub fn known_peers(&self) -> &HashMap<PeerId, Vec<Multiaddr>> {
        &self.peers
    }

    pub fn known_rooms(&self) -> &HashMap<PeerId, RoomAnnouncement> {
        &self.known_rooms
    }

    pub fn set_local_announcement(&mut self, announcement: Option<RoomAnnouncement>) {
        self.local_announcement = announcement;
    }

    /// Drive the event loop. Returns when the `event_tx` receiver is dropped.
    pub async fn run(mut self, event_tx: mpsc::Sender<DiscoveryEvent>) {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    if self.handle_swarm_event(event, &event_tx).await.is_err() {
                        return;
                    }
                }
            }
        }
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<DiscoveryBehaviourEvent>,
        event_tx: &mpsc::Sender<DiscoveryEvent>,
    ) -> Result<(), ()> {
        match event {
            SwarmEvent::Behaviour(DiscoveryBehaviourEvent::Mdns(mdns::Event::Discovered(
                peers,
            ))) => {
                for (peer_id, addr) in peers {
                    self.peers.entry(peer_id).or_default().push(addr);
                    let addresses = self.peers[&peer_id].clone();

                    if let Some(ann) = &self.local_announcement {
                        self.swarm
                            .behaviour_mut()
                            .announce
                            .send_request(&peer_id, ann.clone());
                    }

                    event_tx
                        .send(DiscoveryEvent::PeerDiscovered { peer_id, addresses })
                        .await
                        .map_err(|_| ())?;
                }
            }

            SwarmEvent::Behaviour(DiscoveryBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
                for (peer_id, _) in peers {
                    self.peers.remove(&peer_id);

                    if let Some(room) = self.known_rooms.remove(&peer_id) {
                        event_tx
                            .send(DiscoveryEvent::RoomExpired {
                                peer_id,
                                room_id: room.room_id,
                            })
                            .await
                            .map_err(|_| ())?;
                    }

                    event_tx
                        .send(DiscoveryEvent::PeerExpired { peer_id })
                        .await
                        .map_err(|_| ())?;
                }
            }

            // Inbound: remote peer sent us their RoomAnnouncement.
            SwarmEvent::Behaviour(DiscoveryBehaviourEvent::Announce(AnnounceEvent::Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
                ..
            })) => {
                let response = self.local_announcement.clone();
                let _ = self
                    .swarm
                    .behaviour_mut()
                    .announce
                    .send_response(channel, response);

                if request.protocol_version != PROTOCOL_VERSION {
                    event_tx
                        .send(DiscoveryEvent::ProtocolVersionMismatch {
                            peer_id: peer,
                            their_version: request.protocol_version,
                            our_version: PROTOCOL_VERSION,
                        })
                        .await
                        .map_err(|_| ())?;
                } else {
                    self.known_rooms.insert(peer, request.clone());
                    event_tx
                        .send(DiscoveryEvent::RoomAnnounced {
                            peer_id: peer,
                            announcement: request,
                        })
                        .await
                        .map_err(|_| ())?;
                }
            }

            // Outbound: remote peer replied with their announcement.
            SwarmEvent::Behaviour(DiscoveryBehaviourEvent::Announce(AnnounceEvent::Message {
                peer,
                message:
                    request_response::Message::Response {
                        response: Some(announcement),
                        ..
                    },
                ..
            })) => {
                if announcement.protocol_version != PROTOCOL_VERSION {
                    event_tx
                        .send(DiscoveryEvent::ProtocolVersionMismatch {
                            peer_id: peer,
                            their_version: announcement.protocol_version,
                            our_version: PROTOCOL_VERSION,
                        })
                        .await
                        .map_err(|_| ())?;
                } else {
                    self.known_rooms.insert(peer, announcement.clone());
                    event_tx
                        .send(DiscoveryEvent::RoomAnnounced {
                            peer_id: peer,
                            announcement,
                        })
                        .await
                        .map_err(|_| ())?;
                }
            }

            _ => {}
        }

        Ok(())
    }
}

/// Bridge that runs [`LanDiscovery`] in a background tokio thread.
/// Collects discovered rooms into a shared vec for polling by any UI.
pub struct LanDiscoveryHandle {
    rooms: Arc<Mutex<Vec<RoomAnnouncement>>>,
    _shutdown: mpsc::Sender<()>,
}

impl LanDiscoveryHandle {
    /// Start LAN discovery in a background thread. Returns `None` if the
    /// transport layer cannot be initialised (e.g. no network interfaces).
    pub fn start() -> Option<Self> {
        let announcement =
            RoomAnnouncement::new("", "Meowify Client", RoomVisibility::LanVisible, "user");
        let rooms: Arc<Mutex<Vec<RoomAnnouncement>>> = Arc::new(Mutex::new(Vec::new()));
        let rooms_clone = Arc::clone(&rooms);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let (event_tx, mut event_rx) = mpsc::channel::<DiscoveryEvent>(64);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .build()
                .expect("tokio rt");
            let disco = rt.block_on(async { LanDiscovery::new(Some(announcement)).ok() });
            let Some(discovery) = disco else { return };
            rt.block_on(async move {
                tokio::select! {
                    _ = discovery.run(event_tx) => {}
                    _ = shutdown_rx.recv() => {}
                }
            });
        });

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio rt for event poll");
            rt.block_on(async move {
                while let Some(event) = event_rx.recv().await {
                    if let DiscoveryEvent::RoomAnnounced { announcement, .. } = event {
                        let mut guard = rooms_clone.lock().unwrap();
                        if !guard.iter().any(|r| r.room_id == announcement.room_id) {
                            guard.push(announcement);
                        }
                    }
                }
            });
        });

        Some(Self {
            rooms,
            _shutdown: shutdown_tx,
        })
    }

    /// Snapshot of currently discovered rooms.
    pub fn discovered_rooms(&self) -> Vec<RoomAnnouncement> {
        self.rooms.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RoomVisibility;

    #[test]
    fn room_announcement_carries_current_protocol_version() {
        let ann = RoomAnnouncement::new("room-1", "Test Room", RoomVisibility::LanVisible, "Alice");
        assert_eq!(ann.protocol_version, PROTOCOL_VERSION);
        assert_eq!(ann.room_id, "room-1");
        assert_eq!(ann.room_name, "Test Room");
        assert_eq!(ann.visibility, RoomVisibility::LanVisible);
        assert_eq!(ann.admin_name, "Alice");
    }

    #[test]
    fn private_room_announcement_reflects_visibility() {
        let ann = RoomAnnouncement::new("room-2", "Secret Room", RoomVisibility::Private, "Bob");
        assert_eq!(ann.visibility, RoomVisibility::Private);
    }

    #[test]
    fn room_announcement_roundtrips_json() {
        let ann = RoomAnnouncement::new("room-3", "LAN Party", RoomVisibility::Open, "Carol");
        let json = serde_json::to_string(&ann).unwrap();
        let decoded: RoomAnnouncement = serde_json::from_str(&json).unwrap();
        assert_eq!(ann, decoded);
    }

    #[test]
    fn announcement_with_mismatched_version_is_distinguishable() {
        let ann = RoomAnnouncement {
            room_id: "r1".to_string(),
            room_name: "Room".to_string(),
            visibility: RoomVisibility::LanVisible,
            admin_name: "Host".to_string(),
            protocol_version: PROTOCOL_VERSION + 1,
        };
        assert_ne!(ann.protocol_version, PROTOCOL_VERSION);
    }

    #[tokio::test]
    async fn lan_discovery_constructs_with_and_without_local_announcement() {
        let d1 = LanDiscovery::new(None).expect("no-announcement instance");
        let d2 = LanDiscovery::new(Some(RoomAnnouncement::new(
            "room-x",
            "My Room",
            RoomVisibility::LanVisible,
            "Host",
        )))
        .expect("with-announcement instance");

        assert_ne!(d1.local_peer_id(), d2.local_peer_id());
        assert!(d1.known_peers().is_empty());
        assert!(d1.known_rooms().is_empty());
        assert!(d2.known_rooms().is_empty());
    }
}
