use std::collections::HashMap;
use std::time::Duration;

use libp2p::{
    Multiaddr, PeerId,
    futures::StreamExt as _,
    mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use tokio::sync::mpsc;

use crate::model::{RoomId, RoomVisibility};
use crate::protocol::PROTOCOL_VERSION;

/// Payload broadcast to peers discovered on the LAN.
/// Room announcement exchange (request-response) is a follow-on slice;
/// this type defines what will be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Events emitted by [`LanDiscovery`] as peers appear/disappear on the LAN.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    PeerExpired {
        peer_id: PeerId,
    },
}

#[derive(NetworkBehaviour)]
struct DiscoveryBehaviour {
    mdns: mdns::tokio::Behaviour,
}

/// mDNS-based LAN peer discovery.
///
/// Constructs a libp2p swarm with mDNS behaviour. Call [`run`] to drive the
/// event loop; events are forwarded to the provided `mpsc::Sender`. Room
/// announcement exchange (request-response) is added in a follow-on slice.
///
/// [`run`]: LanDiscovery::run
pub struct LanDiscovery {
    swarm: libp2p::Swarm<DiscoveryBehaviour>,
    peers: HashMap<PeerId, Vec<Multiaddr>>,
}

impl LanDiscovery {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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
                })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(Self {
            swarm,
            peers: HashMap::new(),
        })
    }

    pub fn local_peer_id(&self) -> &PeerId {
        self.swarm.local_peer_id()
    }

    pub fn known_peers(&self) -> &HashMap<PeerId, Vec<Multiaddr>> {
        &self.peers
    }

    /// Drive the mDNS event loop, forwarding peer-discovered/expired events to
    /// `event_tx`. Returns when the channel receiver is dropped.
    pub async fn run(mut self, event_tx: mpsc::Sender<DiscoveryEvent>) {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(DiscoveryBehaviourEvent::Mdns(
                            mdns::Event::Discovered(peers),
                        )) => {
                            for (peer_id, addr) in peers {
                                self.peers.entry(peer_id).or_default().push(addr);
                                let addresses = self.peers[&peer_id].clone();
                                if event_tx
                                    .send(DiscoveryEvent::PeerDiscovered { peer_id, addresses })
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                            }
                        }
                        SwarmEvent::Behaviour(DiscoveryBehaviourEvent::Mdns(
                            mdns::Event::Expired(peers),
                        )) => {
                            for (peer_id, _) in peers {
                                self.peers.remove(&peer_id);
                                if event_tx
                                    .send(DiscoveryEvent::PeerExpired { peer_id })
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
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

    #[tokio::test]
    async fn lan_discovery_constructs_and_has_unique_peer_id() {
        let d1 = LanDiscovery::new().expect("first LanDiscovery::new should succeed");
        let d2 = LanDiscovery::new().expect("second LanDiscovery::new should succeed");
        assert_ne!(
            d1.local_peer_id(),
            d2.local_peer_id(),
            "each instance gets a fresh identity"
        );
        assert!(d1.known_peers().is_empty());
        assert!(d2.known_peers().is_empty());
    }
}
