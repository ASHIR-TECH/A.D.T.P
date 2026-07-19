use adtp_core::types::{PeerIdentity, PEER_ID_SIZE};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::path::Path;

pub const IDENTITY_FILE: &str = "identity.key";

pub struct PeerIdentityKey {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub peer_id: [u8; PEER_ID_SIZE],
}

impl PeerIdentityKey {
    pub fn load_or_generate(path: &Path) -> std::io::Result<Self> {
        let key_bytes = if path.exists() {
            std::fs::read(path)?
        } else {
            let keypair = SigningKey::generate(&mut OsRng);
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
            adtp_version: adtp_core::types::ADTP_VERSION,
            max_file_size: adtp_core::types::ADTP_MAX_FILE_SIZE,
            supported_mime_types: Vec::new(),
            display_name,
        }
    }
}
