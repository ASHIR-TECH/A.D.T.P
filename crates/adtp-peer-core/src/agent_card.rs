use adtp_core::types::PEER_ID_SIZE;
use chrono::Utc;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub adtp_version: u8,
    pub peer_id: String,
    pub public_key_hex: String,
    pub display_name: Option<String>,
    pub host: String,
    pub port: u16,
    pub max_file_size: u64,
    pub supported_mime_types: Vec<String>,
    pub supports_tasks: bool,
    pub supports_multihop: bool,
    pub relay_of: Vec<String>,
    pub issued_at: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsignedCard {
    pub adtp_version: u8,
    pub peer_id: String,
    pub public_key_hex: String,
    pub display_name: Option<String>,
    pub host: String,
    pub port: u16,
    pub max_file_size: u64,
    pub supported_mime_types: Vec<String>,
    pub supports_tasks: bool,
    pub supports_multihop: bool,
    pub relay_of: Vec<String>,
    pub issued_at: String,
}

pub fn build_unsigned_card(
    peer_id: &[u8; PEER_ID_SIZE],
    public_key: &[u8; 32],
    display_name: Option<String>,
    host: &str,
    port: u16,
    max_file_size: u64,
    supported_mime_types: Vec<String>,
    supports_tasks: bool,
    supports_multihop: bool,
    relay_of: Vec<String>,
) -> UnsignedCard {
    UnsignedCard {
        adtp_version: adtp_core::types::ADTP_VERSION,
        peer_id: hex::encode(peer_id),
        public_key_hex: hex::encode(public_key),
        display_name,
        host: host.to_string(),
        port,
        max_file_size,
        supported_mime_types,
        supports_tasks,
        supports_multihop,
        relay_of,
        issued_at: Utc::now().to_rfc3339(),
    }
}

pub fn sign_card(unsigned: &UnsignedCard, signing_key: &SigningKey) -> AgentCard {
    let canonical = serde_json::to_string(unsigned).unwrap_or_default();
    let signature = signing_key.sign(canonical.as_bytes());
    AgentCard {
        adtp_version: unsigned.adtp_version,
        peer_id: unsigned.peer_id.clone(),
        public_key_hex: unsigned.public_key_hex.clone(),
        display_name: unsigned.display_name.clone(),
        host: unsigned.host.clone(),
        port: unsigned.port,
        max_file_size: unsigned.max_file_size,
        supported_mime_types: unsigned.supported_mime_types.clone(),
        supports_tasks: unsigned.supports_tasks,
        supports_multihop: unsigned.supports_multihop,
        relay_of: unsigned.relay_of.clone(),
        issued_at: unsigned.issued_at.clone(),
        signature: hex::encode(signature.to_bytes()),
    }
}

pub fn verify_card(card: &AgentCard) -> Result<bool, String> {
    let public_key_bytes = hex::decode(&card.public_key_hex).map_err(|e| format!("Invalid public_key_hex: {e}"))?;
    let public_key_array: [u8; 32] = public_key_bytes.try_into().map_err(|_| "Public key must be 32 bytes".to_string())?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_array).map_err(|e| format!("Invalid ed25519 key: {e}"))?;

    let signature_bytes = hex::decode(&card.signature).map_err(|e| format!("Invalid signature hex: {e}"))?;
    let sig_array: [u8; 64] = signature_bytes.try_into().map_err(|_| "Signature must be 64 bytes".to_string())?;
    let signature = Signature::from_bytes(&sig_array);

    let unsigned = UnsignedCard {
        adtp_version: card.adtp_version,
        peer_id: card.peer_id.clone(),
        public_key_hex: card.public_key_hex.clone(),
        display_name: card.display_name.clone(),
        host: card.host.clone(),
        port: card.port,
        max_file_size: card.max_file_size,
        supported_mime_types: card.supported_mime_types.clone(),
        supports_tasks: card.supports_tasks,
        supports_multihop: card.supports_multihop,
        relay_of: card.relay_of.clone(),
        issued_at: card.issued_at.clone(),
    };
    let canonical = serde_json::to_string(&unsigned).unwrap_or_default();

    Ok(verifying_key.verify(canonical.as_bytes(), &signature).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_sign_and_verify_card() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let peer_id = [1u8; PEER_ID_SIZE];

        let unsigned = build_unsigned_card(
            &peer_id,
            &verifying_key.to_bytes(),
            Some("test-peer".into()),
            "127.0.0.1",
            40001,
            1073741824,
            vec!["text/plain".into()],
            true,
            false,
            vec![],
        );

        let card = sign_card(&unsigned, &signing_key);
        assert!(!card.signature.is_empty());
        assert!(verify_card(&card).unwrap_or(false));
    }

    #[test]
    fn test_verify_tampered_card_fails() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let peer_id = [2u8; PEER_ID_SIZE];

        let unsigned = build_unsigned_card(
            &peer_id,
            &verifying_key.to_bytes(),
            None,
            "10.0.0.1",
            40001,
            1073741824,
            vec![],
            true,
            true,
            vec!["peer_c".into()],
        );

        let mut card = sign_card(&unsigned, &signing_key);
        card.max_file_size = 999;
        assert!(!verify_card(&card).unwrap_or(true));
    }
}
