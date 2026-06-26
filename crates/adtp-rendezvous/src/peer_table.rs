use adtp_core::types::{NatType, PEER_ID_SIZE, PEER_TABLE_PURGE_SECS};
use chrono::Utc;
use dashmap::DashMap;

struct PeerInfo {
    external_addr: String,
    nat_type: NatType,
    supports_ipv6: bool,
    can_relay: bool,
    last_seen: i64,
}

pub struct PeerTable {
    peers: DashMap<[u8; PEER_ID_SIZE], PeerInfo>,
}

impl PeerTable {
    pub fn new() -> Self {
        let table = Self {
            peers: DashMap::new(),
        };
        let purge_secs = PEER_TABLE_PURGE_SECS;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(purge_secs)).await;
                // Purge handled by get/contains checking freshness
            }
        });
        table
    }

    pub fn register(&self, peer_id: [u8; PEER_ID_SIZE], external_addr: String, nat_type: NatType, supports_ipv6: bool, can_relay: bool) {
        self.peers.insert(peer_id, PeerInfo {
            external_addr,
            nat_type,
            supports_ipv6,
            can_relay,
            last_seen: Utc::now().timestamp(),
        });
    }

    pub fn lookup(&self, peer_id: &[u8; PEER_ID_SIZE]) -> Option<PeerInfoSnapshot> {
        self.peers.get(peer_id).map(|info| {
            let now = Utc::now().timestamp();
            if now - info.last_seen > PEER_TABLE_PURGE_SECS as i64 {
                None
            } else {
                Some(PeerInfoSnapshot {
                    external_addr: info.external_addr.clone(),
                    nat_type: info.nat_type,
                })
            }
        }).flatten()
    }

    pub fn addr(&self, peer_id: &[u8; PEER_ID_SIZE]) -> Option<std::net::SocketAddr> {
        self.peers.get(peer_id).and_then(|info| {
            let now = Utc::now().timestamp();
            if now - info.last_seen > PEER_TABLE_PURGE_SECS as i64 {
                None
            } else {
                info.external_addr.parse::<std::net::SocketAddr>().ok()
            }
        })
    }

    pub fn addr_for_punch(&self, _data: &[u8]) -> Option<std::net::SocketAddr> {
        // Extract target peer_id from PunchReady payload (index 1..17)
        if _data.len() < 18 {
            return None;
        }
        if _data[0] != 0x23 {
            return None;
        }
        let mut peer_id = [0u8; PEER_ID_SIZE];
        peer_id.copy_from_slice(&_data[1..=16]);
        self.addr(&peer_id)
    }

    pub fn refresh(&self, peer_id: &[u8; PEER_ID_SIZE]) {
        if let Some(mut info) = self.peers.get_mut(peer_id) {
            info.last_seen = Utc::now().timestamp();
        }
    }
}

pub struct PeerInfoSnapshot {
    pub external_addr: String,
    pub nat_type: NatType,
}
