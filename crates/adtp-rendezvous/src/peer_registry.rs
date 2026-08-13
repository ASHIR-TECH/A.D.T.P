use adtp_core::types::PEER_ID_SIZE;
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Clone)]
struct KnownPeer {
    public_key: [u8; 32],
    registered_at: i64,
}

pub struct PeerRegistry {
    peers: Arc<DashMap<[u8; PEER_ID_SIZE], KnownPeer>>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(DashMap::new()),
        }
    }

    pub fn register(&self, peer_id: &[u8; PEER_ID_SIZE], public_key: &[u8; 32]) -> bool {
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(mut existing) = self.peers.get_mut(peer_id) {
            if existing.public_key != *public_key {
                tracing::warn!(
                    "Peer {} attempted to register with different public key",
                    hex::encode(peer_id)
                );
                return false;
            }
            existing.registered_at = now;
            return true;
        }
        self.peers.insert(
            *peer_id,
            KnownPeer {
                public_key: *public_key,
                registered_at: now,
            },
        );
        true
    }

    pub fn verify_signature(
        &self,
        peer_id: &[u8; PEER_ID_SIZE],
        signature_hex: &str,
        message: &[u8],
    ) -> bool {
        let sig_bytes = match hex::decode(signature_hex) {
            Ok(b) if b.len() == 64 => {
                let mut arr = [0u8; 64];
                arr.copy_from_slice(&b);
                arr
            }
            _ => {
                tracing::warn!("Invalid signature format from {}", hex::encode(peer_id));
                return false;
            }
        };

        if let Some(peer) = self.peers.get(peer_id) {
            use ed25519_dalek::{Signature, VerifyingKey};
            let verifying_key = VerifyingKey::from_bytes(&peer.public_key);
            match verifying_key {
                Ok(key) => {
                    let sig = Signature::from_bytes(&sig_bytes);
                    key.verify_strict(message, &sig).is_ok()
                }
                Err(e) => {
                    tracing::error!(
                        "Invalid public key for peer {}: {}",
                        hex::encode(peer_id),
                        e
                    );
                    false
                }
            }
        } else {
            tracing::warn!(
                "Unknown peer {} attempted verification",
                hex::encode(peer_id)
            );
            false
        }
    }

    #[allow(dead_code)]
    pub fn known_count(&self) -> usize {
        self.peers.len()
    }
}
