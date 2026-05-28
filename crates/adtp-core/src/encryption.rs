use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

use crate::errors::{AdtpError, AdtpResult};
use crate::types::{AES_KEY_SIZE, AES_NONCE_SIZE};

pub fn derive_session_key(
    shared_secret: &[u8],
    session_id: &[u8],
    info: &[u8],
) -> AdtpResult<[u8; AES_KEY_SIZE]> {
    let hk = Hkdf::<Sha256>::new(Some(session_id), shared_secret);
    let mut key = [0u8; AES_KEY_SIZE];
    hk.expand(info, &mut key)
        .map_err(|e| AdtpError::KeyDerivationFailed {
            reason: e.to_string(),
        })?;
    Ok(key)
}

pub fn generate_nonce() -> [u8; AES_NONCE_SIZE] {
    let mut nonce = [0u8; AES_NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

pub fn encrypt(
    key: &[u8; AES_KEY_SIZE],
    nonce: &[u8; AES_NONCE_SIZE],
    plaintext: &[u8],
) -> AdtpResult<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce);

    cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| AdtpError::EncryptionFailed {
            reason: e.to_string(),
        })
}

pub fn decrypt(
    key: &[u8; AES_KEY_SIZE],
    nonce: &[u8; AES_NONCE_SIZE],
    ciphertext: &[u8],
) -> AdtpResult<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| AdtpError::DecryptionFailed {
            reason: e.to_string(),
        })
}

pub fn encrypt_file_bytes(
    key: &[u8; AES_KEY_SIZE],
    file_bytes: &[u8],
) -> AdtpResult<(Vec<u8>, [u8; AES_NONCE_SIZE])> {
    let nonce = generate_nonce();
    let ciphertext = encrypt(key, &nonce, file_bytes)?;

    Ok((ciphertext, nonce))
}

pub fn decrypt_file_bytes(
    key: &[u8; AES_KEY_SIZE],
    nonce: &[u8; AES_NONCE_SIZE],
    ciphertext: &[u8],
) -> AdtpResult<Vec<u8>> {
    decrypt(key, nonce, ciphertext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [42u8; AES_KEY_SIZE];
        let plaintext = b"Hello Ashir Tech -- ADTP protocol test";
        let (ciphertext, nonce) = encrypt_file_bytes(&key, plaintext).unwrap();
        assert_ne!(&ciphertext, plaintext);

        let decrypted = decrypt_file_bytes(&key, &nonce, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key = [1u8; AES_KEY_SIZE];
        let wrong_key = [2u8; AES_KEY_SIZE];
        let plaintext = b"secret data";
        let (ciphertext, nonce) = encrypt_file_bytes(&key, plaintext).unwrap();

        assert!(decrypt_file_bytes(&wrong_key, &nonce, &ciphertext).is_err());
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = [7u8; AES_KEY_SIZE];
        let plaintext = b"important file";
        let (mut ciphertext, nonce) = encrypt_file_bytes(&key, plaintext).unwrap();
        ciphertext[0] ^= 0xFF;

        assert!(decrypt_file_bytes(&key, &nonce, &ciphertext).is_err());
    }

    #[test]
    fn test_nonce_uniqueness() {
        let n1 = generate_nonce();
        let n2 = generate_nonce();

        assert_ne!(n1, n2);
    }

    #[test]
    fn test_key_derivation() {
        let secret = [0u8; 32];
        let session_id = [1u8; 16];
        let key1 = derive_session_key(&secret, &session_id, b"adtp-session-key").unwrap();
        let key2 = derive_session_key(&secret, &session_id, b"adtp-session-key").unwrap();

        assert_eq!(key1, key2);
    }
}
