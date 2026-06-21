use adtp_core::types::{PeerEntry, TrustLevel, PEER_ID_SIZE};
use std::collections::HashMap;
use std::path::Path;

pub const PEERS_FILE: &str = "peers.toml";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeersConfig {
    pub peer: Vec<PeerEntry>,
}

pub struct TrustStore {
    peers: HashMap<String, PeerEntry>,
    path: std::path::PathBuf,
}

impl TrustStore {
    pub fn load(path: &Path) -> Self {
        let peers = if path.exists() {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|content| toml::from_str::<PeersConfig>(&content).ok())
                .map(|cfg| {
                    cfg.peer
                        .into_iter()
                        .map(|p| (p.peer_id.clone(), p))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            HashMap::new()
        };
        Self {
            peers,
            path: path.to_path_buf(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let config = PeersConfig {
            peer: self.peers.values().cloned().collect(),
        };
        let content = toml::to_string_pretty(&config).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn trust_level(&self, peer_id: &[u8; PEER_ID_SIZE]) -> TrustLevel {
        let hex_id = hex::encode(peer_id);
        match self.peers.get(&hex_id) {
            Some(entry) => entry.trust_level.clone(),
            None => TrustLevel::Unverified,
        }
    }

    pub fn is_allowed(&self, peer_id: &[u8; PEER_ID_SIZE]) -> bool {
        matches!(self.trust_level(peer_id), TrustLevel::Trusted)
    }

    pub fn add_peer(&mut self, entry: PeerEntry) {
        self.peers.insert(entry.peer_id.clone(), entry);
    }

    pub fn remove_peer(&mut self, peer_id: &str) {
        self.peers.remove(peer_id);
    }

    pub fn set_trust(&mut self, peer_id: &str, level: TrustLevel) {
        if let Some(entry) = self.peers.get_mut(peer_id) {
            entry.trust_level = level;
        }
    }

    pub fn list_peers(&self) -> Vec<&PeerEntry> {
        let mut entries: Vec<&PeerEntry> = self.peers.values().collect();
        entries.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
        entries
    }

    pub fn get_peer(&self, peer_id: &str) -> Option<&PeerEntry> {
        self.peers.get(peer_id)
    }
}

pub fn default_peers_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("adtp")
        .join(PEERS_FILE)
}
