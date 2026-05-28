use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use uuid::Uuid;
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::encryption::derive_session_key;
use crate::errors::AdtpResult;
use crate::integrity::generate_keypair;
use crate::types::{Session, AES_KEY_SIZE};

pub struct SessionKeys {
    pub ecdh_secret: EphemeralSecret,
    pub ecdh_public: PublicKey,
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl SessionKeys {
    pub fn generate() -> Self {
        let ecdh_secret = EphemeralSecret::random_from_rng(OsRng);
        let ecdh_public = PublicKey::from(&ecdh_secret);
        let (signing_key, verifying_key) = generate_keypair();

        Self {
            ecdh_secret,
            ecdh_public,
            signing_key,
            verifying_key,
        }
    }

    pub fn complete_handshake(
        self,
        remote_public_key: &[u8; 32],
        session_id: &Uuid,
    ) -> AdtpResult<[u8; AES_KEY_SIZE]> {
        let remote_key = PublicKey::from(*remote_public_key);
        let shared_secret = self.ecdh_secret.diffie_hellman(&remote_key);

        derive_session_key(
            shared_secret.as_bytes(),
            session_id.as_bytes(),
            b"adtp-session-key-v1",
        )
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.ecdh_public.to_bytes()
    }

    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }
}

pub fn new_session(peer_addr: impl Into<String>) -> Session {
    Session {
        id: Uuid::new_v4(),
        created_at: Utc::now(),
        peer_addr: peer_addr.into(),
        symmetric_key: None,
    }
}
