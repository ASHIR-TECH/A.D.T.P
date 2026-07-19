use adtp_core::types::PEER_ID_SIZE;
use chrono::Utc;
use dashmap::DashSet;
use rand::Rng;

pub struct TokenIssuer {
    server_secret: [u8; 32],
    consumed: DashSet<String>,
}

impl TokenIssuer {
    pub fn new() -> Self {
        let mut secret = [0u8; 32];
        rand::thread_rng().fill(&mut secret);
        Self {
            server_secret: secret,
            consumed: DashSet::new(),
        }
    }

    pub fn issue(&self, requester: &[u8; PEER_ID_SIZE], target: &[u8; PEER_ID_SIZE]) -> String {
        let now_ms = Utc::now().timestamp_millis();
        let mut input = Vec::with_capacity(32 + 16 + 16 + 8);
        input.extend_from_slice(&self.server_secret);
        input.extend_from_slice(requester);
        input.extend_from_slice(target);
        input.extend_from_slice(&now_ms.to_le_bytes());
        hex::encode(blake3::hash(&input).as_bytes())
    }

    pub fn validate(&self, token: &str) -> bool {
        !self.consumed.contains(token)
    }

    pub fn consume(&self, token: &str) -> bool {
        if self.consumed.contains(token) {
            return false;
        }
        self.consumed.insert(token.to_string());
        true
    }
}
