use adtp_core::types::{PEER_ID_SIZE, TOKEN_VALIDITY_SECS};
use chrono::Utc;
use dashmap::DashMap;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct TokenRecord {
    issued_at_ms: i64,
    consumed: bool,
}

pub struct TokenIssuer {
    hmac_key: [u8; 32],
    tokens: DashMap<String, TokenRecord>,
}

impl TokenIssuer {
    pub fn new() -> Self {
        let mut hmac_key = [0u8; 32];
        rand::thread_rng().fill(&mut hmac_key);
        Self {
            hmac_key,
            tokens: DashMap::new(),
        }
    }

    fn compute_hmac(
        &self,
        requester: &[u8; PEER_ID_SIZE],
        target: &[u8; PEER_ID_SIZE],
        issued_at_ms: i64,
    ) -> String {
        let mut mac =
            HmacSha256::new_from_slice(&self.hmac_key).expect("HMAC can take key of any size");
        mac.update(requester);
        mac.update(target);
        mac.update(&issued_at_ms.to_le_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    pub fn issue(&self, requester: &[u8; PEER_ID_SIZE], target: &[u8; PEER_ID_SIZE]) -> String {
        let now_ms = Utc::now().timestamp_millis();
        let token = self.compute_hmac(requester, target, now_ms);
        self.tokens.insert(
            token.clone(),
            TokenRecord {
                issued_at_ms: now_ms,
                consumed: false,
            },
        );
        token
    }

    pub fn validate(&self, token: &str) -> bool {
        if let Some(record) = self.tokens.get(token) {
            let now_ms = Utc::now().timestamp_millis();
            let age_secs = (now_ms - record.issued_at_ms) / 1000;
            if age_secs > TOKEN_VALIDITY_SECS as i64 {
                tracing::warn!(
                    "Token expired (age={}s, max={}s)",
                    age_secs,
                    TOKEN_VALIDITY_SECS
                );
                drop(record);
                self.tokens.remove(token);
                return false;
            }
            true
        } else {
            tracing::warn!("Token not recognized (not issued by this server)");
            false
        }
    }

    pub fn consume(&self, token: &str) -> bool {
        if let Some(mut record) = self.tokens.get_mut(token) {
            if record.consumed {
                return false;
            }
            record.consumed = true;
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn cleanup_expired(&self) {
        let now_ms = Utc::now().timestamp_millis();
        self.tokens.retain(|_token, record| {
            let age_secs = (now_ms - record.issued_at_ms) / 1000;
            age_secs <= TOKEN_VALIDITY_SECS as i64
        });
    }
}
