//! # adtp-core
//!
//! The foundational layer of ADTP — Agent Direct Transfer Protocol.
//!
//! ## What this crate provides
//! - `types` — All shared types.
//! - `errors` — All error types.
//! - `protocol` — ADTP packet structure and serialisation.
//! - `integrity` — BLAKE3 hashing and ed25519 signing.
//! - `encryption` — AES-256-GCM encryption/decryption.
//! - `session` — Session and keypair management.
//! - `transfer` — QUIC transport engine.
//!
//! ## Example
//! ```rust
//! use adtp_core::encryption::{decrypt_file_bytes, encrypt_file_bytes};
//!
//! let key = [0u8; 32];
//! let file_data = b"Hello from Ashir Tech";
//! let (ciphertext, nonce) = encrypt_file_bytes(&key, file_data).unwrap();
//! let decrypted = decrypt_file_bytes(&key, &nonce, &ciphertext).unwrap();
//! assert_eq!(file_data, decrypted.as_slice());
//! ```

pub mod encryption;
pub mod errors;
pub mod integrity;
pub mod protocol;
pub mod session;
pub mod tofu;
pub mod transfer;
pub mod types;

pub use errors::{AdtpError, AdtpResult};
pub use protocol::{AdtpPacket, ContextEnvelope, PacketHeader};
pub use types::*;
