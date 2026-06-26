use adtp_core::types::PEER_ID_SIZE;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeSignal {
    pub requester_peer_id: [u8; PEER_ID_SIZE],
    pub token: String,
}

pub struct WakeService;

impl WakeService {
    pub fn new() -> Self {
        Self
    }

    pub fn build_wake(&self, requester: &[u8; PEER_ID_SIZE], token: &str) -> Vec<u8> {
        let signal = WakeSignal {
            requester_peer_id: *requester,
            token: token.to_string(),
        };
        let mut bytes = vec![0x24u8]; // Wake signal type
        if let Ok(json) = serde_json::to_vec(&signal) {
            bytes.extend(json);
        }
        bytes
    }
}
