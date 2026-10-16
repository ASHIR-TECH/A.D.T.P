use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use uuid::Uuid;
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::encryption::derive_session_key;
use crate::errors::AdtpResult;
use crate::integrity::generate_keypair;
use crate::types::{PeerIdentity, Session, AES_KEY_SIZE, PEER_ID_SIZE};

pub const IDENTITY_FILE: &str = "identity.key";

pub struct PersistentPeerIdentity {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub peer_id: [u8; PEER_ID_SIZE],
}

impl PersistentPeerIdentity {
    pub fn load_or_generate(path: &std::path::Path) -> std::io::Result<Self> {
        let key_bytes = if path.exists() {
            std::fs::read(path)?
        } else {
            let keypair = ed25519_dalek::SigningKey::generate(&mut OsRng);
            let bytes = keypair.to_keypair_bytes().to_vec();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &bytes)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            }
            bytes
        };

        let signing_key = SigningKey::from_keypair_bytes(&key_bytes.try_into().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid key file")
        })?)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let verifying_key = signing_key.verifying_key();
        let public_bytes = verifying_key.to_bytes();
        let hash = blake3::hash(&public_bytes);
        let mut peer_id = [0u8; PEER_ID_SIZE];
        peer_id.copy_from_slice(&hash.as_bytes()[..PEER_ID_SIZE]);

        Ok(Self {
            signing_key,
            verifying_key,
            peer_id,
        })
    }

    pub fn to_peer_identity(&self, display_name: Option<String>) -> PeerIdentity {
        PeerIdentity {
            peer_id: self.peer_id,
            public_key: self.verifying_key.to_bytes(),
            adtp_version: crate::types::ADTP_VERSION,
            max_file_size: crate::types::ADTP_MAX_FILE_SIZE,
            supported_mime_types: Vec::new(),
            display_name,
        }
    }
}

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

pub async fn client_handshake(
    connection: &crate::transfer::AdtpConnection,
    session_id: &Uuid,
) -> crate::errors::AdtpResult<[u8; AES_KEY_SIZE]> {
    use crate::transfer::write_raw;

    let keys = SessionKeys::generate();
    let my_public = keys.public_key_bytes();

    let (mut send, mut recv) =
        connection
            .open_bi()
            .await
            .map_err(|e| crate::errors::AdtpError::QuicStreamError {
                reason: e.to_string(),
            })?;

    write_raw(&mut send, &my_public).await?;
    send.finish()
        .map_err(|e| crate::errors::AdtpError::QuicStreamError {
            reason: e.to_string(),
        })?;

    let mut peer_public = [0u8; 32];
    recv.read_exact(&mut peer_public).await.map_err(|e| {
        crate::errors::AdtpError::QuicStreamError {
            reason: e.to_string(),
        }
    })?;

    keys.complete_handshake(&peer_public, session_id)
}

pub async fn server_handshake(
    connection: &crate::transfer::AdtpConnection,
    session_id: &Uuid,
) -> crate::errors::AdtpResult<[u8; AES_KEY_SIZE]> {
    use crate::transfer::write_raw;

    let keys = SessionKeys::generate();
    let my_public = keys.public_key_bytes();

    let (mut send, mut recv) =
        connection
            .accept_bi()
            .await
            .map_err(|e| crate::errors::AdtpError::QuicStreamError {
                reason: e.to_string(),
            })?;

    let mut peer_public = [0u8; 32];
    recv.read_exact(&mut peer_public).await.map_err(|e| {
        crate::errors::AdtpError::QuicStreamError {
            reason: e.to_string(),
        }
    })?;

    write_raw(&mut send, &my_public).await?;

    keys.complete_handshake(&peer_public, session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_keys_generate_unique() {
        let k1 = SessionKeys::generate();
        let k2 = SessionKeys::generate();
        assert_ne!(k1.public_key_bytes(), k2.public_key_bytes());
    }

    #[test]
    fn test_ecdh_derives_same_key_both_sides() {
        let client_keys = SessionKeys::generate();
        let server_keys = SessionKeys::generate();
        let session_id = Uuid::new_v4();

        let client_pub = client_keys.public_key_bytes();
        let server_pub = server_keys.public_key_bytes();

        let client_key = client_keys
            .complete_handshake(&server_pub, &session_id)
            .unwrap();
        let server_key = server_keys
            .complete_handshake(&client_pub, &session_id)
            .unwrap();

        assert_eq!(client_key, server_key);
    }

    #[test]
    fn test_ecdh_wrong_key_yields_different_result() {
        let client_keys = SessionKeys::generate();
        let server_keys = SessionKeys::generate();
        let attacker_keys = SessionKeys::generate();
        let session_id = Uuid::new_v4();

        let client_pub = client_keys.public_key_bytes();
        let server_pub = server_keys.public_key_bytes();
        let attacker_pub = attacker_keys.public_key_bytes();

        let real_key = client_keys
            .complete_handshake(&server_pub, &session_id)
            .unwrap();
        let wrong_key = attacker_keys
            .complete_handshake(&client_pub, &session_id)
            .unwrap();

        assert_ne!(real_key, wrong_key);

        let _ = real_key;
        let _ = wrong_key;
        let _ = client_pub;
        let _ = attacker_pub;
    }
}
