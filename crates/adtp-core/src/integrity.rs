use blake3::Hasher;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

use crate::errors::{AdtpError, AdtpResult};
use crate::types::BLAKE3_HASH_SIZE;

pub fn hash_bytes(data: &[u8]) -> [u8; BLAKE3_HASH_SIZE] {
    *blake3::hash(data).as_bytes()
}

pub async fn hash_file(path: &std::path::Path) -> AdtpResult<[u8; BLAKE3_HASH_SIZE]> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| AdtpError::FileNotFound {
            path: path.to_string_lossy().to_string(),
        })?;
    let mut hasher = Hasher::new();
    let mut buffer = vec![0u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(*hasher.finalize().as_bytes())
}

pub fn verify_hash(data: &[u8], expected: &[u8; BLAKE3_HASH_SIZE]) -> AdtpResult<()> {
    let actual = hash_bytes(data);
    if &actual != expected {
        return Err(AdtpError::HashMismatch {
            expected: hex::encode(expected),
            got: hex::encode(actual),
        });
    }
    Ok(())
}

pub fn hash_to_hex(hash: &[u8; BLAKE3_HASH_SIZE]) -> String {
    hex::encode(hash)
}

pub fn generate_keypair() -> (SigningKey, VerifyingKey) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

pub fn sign(data: &[u8], signing_key: &SigningKey) -> [u8; 64] {
    signing_key.sign(data).to_bytes()
}

pub fn verify_signature(
    data: &[u8],
    signature_bytes: &[u8; 64],
    verifying_key: &VerifyingKey,
) -> AdtpResult<()> {
    let signature = Signature::from_bytes(signature_bytes);
    verifying_key
        .verify(data, &signature)
        .map_err(|_| AdtpError::SignatureInvalid)
}

pub fn compute_chain_hash(
    previous_hash: &[u8; BLAKE3_HASH_SIZE],
    entry_data: &[u8],
) -> [u8; BLAKE3_HASH_SIZE] {
    let mut hasher = Hasher::new();
    hasher.update(previous_hash);
    hasher.update(entry_data);
    *hasher.finalize().as_bytes()
}

pub fn genesis_hash() -> [u8; BLAKE3_HASH_SIZE] {
    hash_bytes(b"ADTP_GENESIS_ASHIR_TECH_2026")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_bytes_deterministic() {
        let data = b"ADTP Ashir Tech 2026";
        let hash1 = hash_bytes(data);
        let hash2 = hash_bytes(data);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_verify_pass() {
        let data = b"test file content";
        let hash = hash_bytes(data);

        assert!(verify_hash(data, &hash).is_ok());
    }

    #[test]
    fn test_hash_verify_tampered() {
        let data = b"original content";
        let hash = hash_bytes(data);
        let tampered = b"tampered content";

        assert!(verify_hash(tampered, &hash).is_err());
    }

    #[test]
    fn test_sign_and_verify() {
        let (signing_key, verifying_key) = generate_keypair();
        let data = b"sign this packet";
        let sig = sign(data, &signing_key);

        assert!(verify_signature(data, &sig, &verifying_key).is_ok());
    }

    #[test]
    fn test_verify_wrong_key_fails() {
        let (signing_key, _) = generate_keypair();
        let (_, wrong_verifying_key) = generate_keypair();
        let data = b"sign this";
        let sig = sign(data, &signing_key);

        assert!(verify_signature(data, &sig, &wrong_verifying_key).is_err());
    }

    #[test]
    fn test_audit_chain() {
        let genesis = genesis_hash();
        let entry1 = b"first audit entry";
        let hash1 = compute_chain_hash(&genesis, entry1);
        let entry2 = b"second audit entry";
        let hash2 = compute_chain_hash(&hash1, entry2);

        assert_ne!(hash1, hash2);
        assert_eq!(hash2, compute_chain_hash(&hash1, entry2));
    }
}
