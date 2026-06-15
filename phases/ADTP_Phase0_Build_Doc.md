**ADTP — Agent Direct Transfer Protocol**  
**Phase 0 Build Documentation**  
**Foundation Layer — Core Protocol**  
**Company:** Ashir Tech (Ashir Group)  
   
 **Protocol:** ADTP v0.1.0  
   
 **Transport:** QUIC (via quinn crate)  
   
 **Language:** Rust 2021 Edition  
   
 **Phase:** 0 of 8  
   
 **Status:** Active Development  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AABAAsSNBCUrfD6LYGNDAgAU2QtIq6DIzW7UHAMBfHGt1V+fXEwAAXrseHDAF/orRG+cAAAAASUVORK5CYII=)  
**OVERVIEW**  
Phase 0 builds the entire foundation of ADTP. Nothing else can be built without this phase being solid. You are building:  
1. All shared types every other crate uses  
2. All error types  
3. The ADTP packet structure (what travels over the wire)  
4. BLAKE3 file integrity hashing  
5. AES-256-GCM encryption and decryption  
6. The QUIC transport layer  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAAM0lEQVR4nO3OQQmAUBBAwSeILbyYdDP8jAaxgjcRZhLMNjNntQIA4C/uvTqq6+sJAADvPS2NA0FrXqf/AAAAAElFTkSuQmCC)  
**STEP 1 — SET UP CARGO WORKSPACE**  
Before writing any code, configure the Rust workspace so all crates can share dependencies.  
**File: **Cargo.toml ** (root — workspace manifest)**  
[workspace]  
 members = [  
     "crates/adtp-core",  
     "crates/adtp-client",  
     "crates/adtp-server",  
     "crates/adtp-cli",  
     "crates/adtp-proto",  
 ]  
 resolver = "2"  
   
 # Shared dependencies — define versions once, use everywhere  
 [workspace.dependencies]  
   
 # Async runtime  
 tokio = { version = "1.36", features = ["full"] }  
   
 # QUIC transport — this is the core transport layer  
 quinn = "0.11"  
   
 # TLS for QUIC  
 rustls = { version = "0.23", features = ["ring"] }  
 rustls-pemfile = "2"  
   
 # Cryptography  
 aes-gcm = "0.10"           # AES-256-GCM encryption  
 blake3 = "1.5"             # BLAKE3 hashing (faster than SHA-256)  
 ed25519-dalek = "2"        # ed25519 digital signatures  
 x25519-dalek = "2"         # Key exchange  
 rand = "0.8"               # Cryptographic random number generation  
 sha2 = "0.10"              # SHA-256 (for key derivation)  
 hkdf = "0.12"              # HKDF key derivation  
   
 # Serialisation  
 serde = { version = "1", features = ["derive"] }  
 serde_json = "1"  
 bincode = "1"              # Binary serialisation for packets  
 bytes = "1"                # Efficient byte buffer manipulation  
 prost = "0.12"             # Protobuf (for adtp-proto)  
   
 # Error handling  
 anyhow = "1"  
 thiserror = "1"  
   
 # Logging and tracing  
 tracing = "0.1"  
 tracing-subscriber = { version = "0.3", features = ["env-filter"] }  
   
 # Time  
 chrono = { version = "0.4", features = ["serde"] }  
   
 # UUID  
 uuid = { version = "1", features = ["v4", "serde"] }  
   
 # Config  
 toml = "0.8"  
   
 # Async traits  
 async-trait = "0.1"  
   
 [profile.release]  
 opt-level = 3  
 lto = true  
 codegen-units = 1  
 strip = true  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSeYxZw/lVeDGMACBrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA6fOBdd+dKAKAAAAAElFTkSuQmCC)  
**STEP 2 — SET UP adtp-core CRATE**  
This is the most important crate. Every other crate depends on it.  
**File: **crates/adtp-core/Cargo.toml  
[package]  
 name = "adtp-core"  
 version = "0.1.0"  
 edition = "2021"  
 description = "ADTP core protocol — packet structure, encryption, integrity, QUIC transport"  
 license = "MIT"  
   
 [dependencies]  
 # All from workspace  
 tokio = { workspace = true }  
 quinn = { workspace = true }  
 rustls = { workspace = true }  
 rustls-pemfile = { workspace = true }  
 aes-gcm = { workspace = true }  
 blake3 = { workspace = true }  
 ed25519-dalek = { workspace = true }  
 x25519-dalek = { workspace = true }  
 rand = { workspace = true }  
 sha2 = { workspace = true }  
 hkdf = { workspace = true }  
 serde = { workspace = true }  
 serde_json = { workspace = true }  
 bincode = { workspace = true }  
 bytes = { workspace = true }  
 anyhow = { workspace = true }  
 thiserror = { workspace = true }  
 tracing = { workspace = true }  
 chrono = { workspace = true }  
 uuid = { workspace = true }  
 async-trait = { workspace = true }  
   
 [dev-dependencies]  
 tokio-test = "0.4"  
 tempfile = "3"  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsScYxpg/h5VMYARvRrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA224BcUMk6pDAAAAAElFTkSuQmCC)  
**STEP 3 — BUILD **types.rs  
**File:** crates/adtp-core/src/types.rs  
This is the first file you write. It defines the language of the entire protocol. Every struct, enum and constant that the whole project shares lives here.  
**What to build:**  
use serde::{Deserialize, Serialize};  
 use uuid::Uuid;  
 use chrono::{DateTime, Utc};  
 use bytes::Bytes;  
   
 // ─── PROTOCOL CONSTANTS ───────────────────────────────────────────────────────  
   
 /// Magic bytes that identify every ADTP packet.  
 /// Spells "ADTP" in hex. Used to validate incoming packets.  
 pub const ADTP_MAGIC: u32 = 0xAD7F1A00;  
   
 /// Current protocol version. Increment when breaking changes occur.  
 pub const ADTP_VERSION: u8 = 1;  
   
 /// Default port the ADTP server daemon listens on  
 pub const ADTP_DEFAULT_PORT: u16 = 40001;  
   
 /// Maximum file size for a single transfer (10 GB)  
 pub const ADTP_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024 * 1024;  
   
 /// Maximum context block size (1 MB)  
 pub const ADTP_MAX_CONTEXT_SIZE: usize = 1 * 1024 * 1024;  
   
 /// Size of AES-256-GCM key in bytes  
 pub const AES_KEY_SIZE: usize = 32;  
   
 /// Size of AES-256-GCM nonce in bytes  
 pub const AES_NONCE_SIZE: usize = 12;  
   
 /// Size of AES-256-GCM authentication tag  
 pub const AES_TAG_SIZE: usize = 16;  
   
 /// Size of ed25519 signature in bytes  
 pub const ED25519_SIG_SIZE: usize = 64;  
   
 /// Size of BLAKE3 hash in bytes  
 pub const BLAKE3_HASH_SIZE: usize = 32;  
   
 // ─── TRANSFER TYPES ───────────────────────────────────────────────────────────  
   
 /// Every ADTP packet has a transfer type.  
 /// This tells the receiver what to do with the packet.  
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]  
 #[repr(u8)]  
 pub enum TransferType {  
     /// User sends a file to the AI agent  
     Send = 0x01,  
   
     /// AI agent pushes a file to the user machine  
     Push = 0x02,  
   
     /// Receiver confirms file arrived and integrity verified  
     Ack = 0x03,  
   
     /// Transfer failed — contains error reason  
     Error = 0x04,  
   
     /// Register a folder for automatic file watching  
     Watch = 0x05,  
   
     /// Remove a folder from watch list  
     Unwatch = 0x06,  
   
     /// Query the transfer queue status  
     Status = 0x07,  
   
     /// Request audit history  
     History = 0x08,  
   
     /// Session handshake — exchange public keys  
     Handshake = 0x09,  
   
     /// Keepalive ping  
     Ping = 0x0A,  
   
     /// Keepalive pong  
     Pong = 0x0B,  
 }  
   
 // ─── TRANSFER PRIORITY ────────────────────────────────────────────────────────  
   
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]  
 #[repr(u8)]  
 pub enum Priority {  
     Low = 0x00,  
     Normal = 0x01,  
     Urgent = 0x02,  
     Critical = 0x03,  
 }  
   
 impl Default for Priority {  
     fn default() -> Self {  
         Priority::Normal  
     }  
 }  
   
 // ─── TRANSFER STATUS ──────────────────────────────────────────────────────────  
   
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]  
 pub enum TransferStatus {  
     /// Queued and waiting  
     Pending,  
     /// Actively being transmitted  
     InProgress { bytes_sent: u64, total_bytes: u64 },  
     /// Successfully delivered and verified  
     Delivered,  
     /// Failed with reason  
     Failed { reason: String },  
     /// Cancelled by sender  
     Cancelled,  
 }  
   
 // ─── FILE METADATA ────────────────────────────────────────────────────────────  
   
 /// Metadata about the file being transferred.  
 /// This is NOT the context block — this is just basic file info.  
 #[derive(Debug, Clone, Serialize, Deserialize)]  
 pub struct FileMeta {  
     /// Original filename with extension  
     pub filename: String,  
   
     /// MIME type detected from file contents (not extension)  
     pub mime_type: String,  
   
     /// File size in bytes  
     pub file_size: u64,  
   
     /// File extension (lowercase, without dot)  
     pub extension: String,  
   
     /// Last modified timestamp of the original file  
     pub last_modified: Option<DateTime<Utc>>,  
   
     /// BLAKE3 hash of the raw (unencrypted) file bytes  
     pub content_hash: [u8; BLAKE3_HASH_SIZE],  
 }  
   
 // ─── CONTEXT BLOCK ────────────────────────────────────────────────────────────  
   
 /// The intelligence layer of ADTP.  
 /// Every file transfer carries a context block so the AI agent  
 /// knows what it is receiving BEFORE processing the bytes.  
 #[derive(Debug, Clone, Serialize, Deserialize)]  
 pub struct ContextBlock {  
     /// Detected file schema (columns for Excel, headings for PDF, etc.)  
     pub schema: Option<serde_json::Value>,  
   
     /// Human-readable summary of file contents  
     pub summary: Option<String>,  
   
     /// Suggested action for the agent to take with this file  
     pub agent_hint: Option<String>,  
   
     /// History of previous transfers of similar files in this session  
     pub transfer_history: Vec<TransferHistoryEntry>,  
   
     /// Custom key-value metadata attached by the sender  
     pub custom_metadata: std::collections::HashMap<String, String>,  
 }  
   
 impl Default for ContextBlock {  
     fn default() -> Self {  
         Self {  
             schema: None,  
             summary: None,  
             agent_hint: None,  
             transfer_history: Vec::new(),  
             custom_metadata: std::collections::HashMap::new(),  
         }  
     }  
 }  
   
 // ─── TRANSFER HISTORY ENTRY ───────────────────────────────────────────────────  
   
 #[derive(Debug, Clone, Serialize, Deserialize)]  
 pub struct TransferHistoryEntry {  
     /// When this file (or similar) was last transferred  
     pub timestamp: DateTime<Utc>,  
   
     /// What the agent did with it last time  
     pub agent_action: Option<String>,  
   
     /// MIME type of the historically transferred file  
     pub mime_type: String,  
 }  
   
 // ─── SESSION ──────────────────────────────────────────────────────────────────  
   
 /// A unique ADTP session between one client and one server.  
 /// Sessions are ephemeral — new keypairs generated every connection.  
 #[derive(Debug, Clone, Serialize, Deserialize)]  
 pub struct Session {  
     /// Unique session identifier  
     pub id: Uuid,  
   
     /// When the session was established  
     pub created_at: DateTime<Utc>,  
   
     /// Remote peer address  
     pub peer_addr: String,  
   
     /// Session-level AES key (derived from ECDH key exchange)  
     #[serde(skip)]  
     pub symmetric_key: Option<[u8; AES_KEY_SIZE]>,  
 }  
   
 // ─── TRANSFER RECORD ──────────────────────────────────────────────────────────  
   
 /// A complete record of one file transfer. Used in audit log.  
 #[derive(Debug, Clone, Serialize, Deserialize)]  
 pub struct TransferRecord {  
     /// Unique transfer identifier  
     pub id: Uuid,  
   
     /// Session this transfer belongs to  
     pub session_id: Uuid,  
   
     /// Direction of transfer  
     pub transfer_type: TransferType,  
   
     /// File metadata  
     pub file_meta: FileMeta,  
   
     /// Current status  
     pub status: TransferStatus,  
   
     /// When transfer was initiated  
     pub initiated_at: DateTime<Utc>,  
   
     /// When transfer completed (if done)  
     pub completed_at: Option<DateTime<Utc>>,  
   
     /// ed25519 signature of the packet (hex encoded)  
     pub signature: String,  
   
     /// BLAKE3 hash of received bytes (for verification)  
     pub received_hash: Option<[u8; BLAKE3_HASH_SIZE]>,  
   
     /// Whether integrity check passed  
     pub integrity_verified: bool,  
 }  
   
 // ─── WATCH ENTRY ──────────────────────────────────────────────────────────────  
   
 /// A registered folder watch on the client machine.  
 #[derive(Debug, Clone, Serialize, Deserialize)]  
 pub struct WatchEntry {  
     pub id: Uuid,  
     pub folder_path: String,  
     pub file_pattern: Option<String>,  
     pub trigger_action: Option<String>,  
     pub created_at: DateTime<Utc>,  
     pub active: bool,  
 }  
   
 // ─── ADTP ADDRESSES ───────────────────────────────────────────────────────────  
   
 /// A fully qualified ADTP endpoint address.  
 #[derive(Debug, Clone, Serialize, Deserialize)]  
 pub struct AdtpAddr {  
     pub host: String,  
     pub port: u16,  
 }  
   
 impl AdtpAddr {  
     pub fn new(host: impl Into<String>, port: u16) -> Self {  
         Self { host: host.into(), port }  
     }  
   
     pub fn to_socket_addr(&self) -> String {  
         format!("{}:{}", self.host, self.port)  
     }  
 }  
   
 impl Default for AdtpAddr {  
     fn default() -> Self {  
         Self {  
             host: "127.0.0.1".to_string(),  
             port: ADTP_DEFAULT_PORT,  
         }  
     }  
 }  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSfYxZo/jVEMYQLPJrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA4rLBc059ysnAAAAAElFTkSuQmCC)  
**STEP 4 — BUILD **errors.rs  
**File:** crates/adtp-core/src/errors.rs  
Every possible failure in ADTP is defined here. Using thiserror for clean, typed errors.  
use thiserror::Error;  
   
 #[derive(Debug, Error)]  
 pub enum AdtpError {  
     // ── Protocol Errors ───────────────────────────────────────────────────────  
   
     #[error("Invalid magic bytes: expected 0xAD7F1A00, got {got:#010x}")]  
     InvalidMagic { got: u32 },  
   
     #[error("Unsupported protocol version: {version}")]  
     UnsupportedVersion { version: u8 },  
   
     #[error("Packet too large: {size} bytes exceeds maximum {max} bytes")]  
     PacketTooLarge { size: u64, max: u64 },  
   
     #[error("Invalid transfer type byte: {byte:#04x}")]  
     InvalidTransferType { byte: u8 },  
   
     #[error("Malformed packet: {reason}")]  
     MalformedPacket { reason: String },  
   
     // ── Integrity Errors ──────────────────────────────────────────────────────  
   
     #[error("Integrity check failed: hash mismatch. Expected {expected}, got {got}")]  
     HashMismatch { expected: String, got: String },  
   
     #[error("Signature verification failed")]  
     SignatureInvalid,  
   
     #[error("File tampered: content hash does not match header")]  
     FileTampered,  
   
     // ── Encryption Errors ─────────────────────────────────────────────────────  
   
     #[error("Encryption failed: {reason}")]  
     EncryptionFailed { reason: String },  
   
     #[error("Decryption failed: {reason}")]  
     DecryptionFailed { reason: String },  
   
     #[error("Invalid key size: expected {expected} bytes, got {got}")]  
     InvalidKeySize { expected: usize, got: usize },  
   
     #[error("Invalid nonce size: expected {expected} bytes, got {got}")]  
     InvalidNonceSize { expected: usize, got: usize },  
   
     #[error("Key derivation failed: {reason}")]  
     KeyDerivationFailed { reason: String },  
   
     // ── Transfer Errors ───────────────────────────────────────────────────────  
   
     #[error("File not found: {path}")]  
     FileNotFound { path: String },  
   
     #[error("File too large: {size} bytes exceeds limit of {limit} bytes")]  
     FileTooLarge { size: u64, limit: u64 },  
   
     #[error("Transfer already in progress for session {session_id}")]  
     TransferInProgress { session_id: String },  
   
     #[error("Transfer not found: {transfer_id}")]  
     TransferNotFound { transfer_id: String },  
   
     #[error("Transfer cancelled by remote")]  
     TransferCancelled,  
   
     #[error("Transfer timeout after {seconds}s")]  
     TransferTimeout { seconds: u64 },  
   
     // ── Session Errors ────────────────────────────────────────────────────────  
   
     #[error("Session not found: {session_id}")]  
     SessionNotFound { session_id: String },  
   
     #[error("Session expired")]  
     SessionExpired,  
   
     #[error("Handshake failed: {reason}")]  
     HandshakeFailed { reason: String },  
   
     #[error("Authentication failed: invalid key")]  
     AuthenticationFailed,  
   
     // ── QUIC Transport Errors ─────────────────────────────────────────────────  
   
     #[error("QUIC connection failed: {reason}")]  
     QuicConnectionFailed { reason: String },  
   
     #[error("QUIC stream error: {reason}")]  
     QuicStreamError { reason: String },  
   
     #[error("Connection closed by remote")]  
     ConnectionClosed,  
   
     #[error("TLS error: {reason}")]  
     TlsError { reason: String },  
   
     #[error("Bind failed on {addr}: {reason}")]  
     BindFailed { addr: String, reason: String },  
   
     // ── IO Errors ─────────────────────────────────────────────────────────────  
   
     #[error("IO error: {0}")]  
     Io(#[from] std::io::Error),  
   
     #[error("Serialisation error: {0}")]  
     Serialisation(#[from] bincode::Error),  
   
     #[error("JSON error: {0}")]  
     Json(#[from] serde_json::Error),  
   
     // ── Context Errors ────────────────────────────────────────────────────────  
   
     #[error("Context extraction failed for {mime_type}: {reason}")]  
     ContextExtractionFailed { mime_type: String, reason: String },  
   
     #[error("Context block too large: {size} bytes")]  
     ContextTooLarge { size: usize },  
   
     // ── Watch Errors ──────────────────────────────────────────────────────────  
   
     #[error("Folder not found: {path}")]  
     FolderNotFound { path: String },  
   
     #[error("Watch already registered for: {path}")]  
     WatchAlreadyRegistered { path: String },  
   
     #[error("Watch not found: {watch_id}")]  
     WatchNotFound { watch_id: String },  
   
     // ── General ───────────────────────────────────────────────────────────────  
   
     #[error("Internal error: {reason}")]  
     Internal { reason: String },  
   
     #[error(transparent)]  
     Other(#[from] anyhow::Error),  
 }  
   
 /// Convenience type alias  
 pub type AdtpResult<T> = Result<T, AdtpError>;  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AABAAsSNBCkLfE07YGfHAiAU2QtIq6DIzW7UHAMBfnGt1V8fXEwAAXrse4eQF6VhvmPsAAAAASUVORK5CYII=)  
**STEP 5 — BUILD **protocol.rs  
**File:** crates/adtp-core/src/protocol.rs  
This is the heart of ADTP. This defines exactly what travels over the QUIC tunnel.  
use serde::{Deserialize, Serialize};  
 use bytes::{Bytes, BytesMut, BufMut, Buf};  
 use uuid::Uuid;  
 use chrono::Utc;  
   
 use crate::types::*;  
 use crate::errors::{AdtpError, AdtpResult};  
   
 // ─── ADTP PACKET ──────────────────────────────────────────────────────────────  
 //  
 // Wire format (binary, big-endian):  
 //  
 // ┌─────────────────────────────────────────────────────────────┐  
 // │ HEADER (fixed 80 bytes)                                     │  
 // │  magic:       4 bytes   — 0xAD7F1A00                        │  
 // │  version:     1 byte    — protocol version                  │  
 // │  transfer_type: 1 byte  — TransferType enum                 │  
 // │  priority:    1 byte    — Priority enum                     │  
 // │  flags:       1 byte    — reserved for future use           │  
 // │  session_id:  16 bytes  — UUID v4                           │  
 // │  transfer_id: 16 bytes  — UUID v4                           │  
 // │  timestamp:   8 bytes   — unix nanoseconds (u64)            │  
 // │  context_len: 4 bytes   — context block byte length (u32)   │  
 // │  payload_len: 8 bytes   — encrypted payload byte length(u64)│  
 // │  nonce:       12 bytes  — AES-256-GCM nonce                 │  
 // │  content_hash: 32 bytes — BLAKE3 of raw (pre-encrypt) bytes │  <- wait, that's already 104 bytes  
 // │  (pad to 128 bytes total for alignment)                     │  
 // ├─────────────────────────────────────────────────────────────┤  
 // │ CONTEXT BLOCK (variable, plaintext JSON)                    │  
 // │  FileMeta + ContextBlock serialised as JSON                 │  
 // ├─────────────────────────────────────────────────────────────┤  
 // │ PAYLOAD (variable, AES-256-GCM encrypted)                   │  
 // │  Raw file bytes, encrypted                                  │  
 // ├─────────────────────────────────────────────────────────────┤  
 // │ SIGNATURE (64 bytes, ed25519)                               │  
 // │  Signs: header bytes + BLAKE3(context) + BLAKE3(payload)    │  
 // └─────────────────────────────────────────────────────────────┘  
   
 /// The ADTP packet header — fixed size, always first on the wire.  
 #[derive(Debug, Clone, Serialize, Deserialize)]  
 pub struct PacketHeader {  
     /// Always 0xAD7F1A00 — validates this is an ADTP packet  
     pub magic: u32,  
   
     /// Protocol version — reject if incompatible  
     pub version: u8,  
   
     /// What kind of transfer this is  
     pub transfer_type: TransferType,  
   
     /// Transfer priority  
     pub priority: Priority,  
   
     /// Reserved flags byte (set to 0x00 for now)  
     pub flags: u8,  
   
     /// Session this packet belongs to  
     pub session_id: Uuid,  
   
     /// Unique ID for this specific transfer  
     pub transfer_id: Uuid,  
   
     /// Packet creation time (unix nanoseconds)  
     pub timestamp_ns: u64,  
   
     /// Byte length of the context block that follows the header  
     pub context_len: u32,  
   
     /// Byte length of the encrypted payload that follows context  
     pub payload_len: u64,  
   
     /// AES-256-GCM nonce used to encrypt this payload  
     pub nonce: [u8; 12],  
   
     /// BLAKE3 hash of the raw (unencrypted) file bytes  
     pub content_hash: [u8; 32],  
 }  
   
 impl PacketHeader {  
     /// Create a new packet header  
     pub fn new(  
         transfer_type: TransferType,  
         session_id: Uuid,  
         transfer_id: Uuid,  
         nonce: [u8; 12],  
         content_hash: [u8; 32],  
         context_len: u32,  
         payload_len: u64,  
     ) -> Self {  
         Self {  
             magic: ADTP_MAGIC,  
             version: ADTP_VERSION,  
             transfer_type,  
             priority: Priority::Normal,  
             flags: 0x00,  
             session_id,  
             transfer_id,  
             timestamp_ns: chrono::Utc::now()  
                 .timestamp_nanos_opt()  
                 .unwrap_or(0) as u64,  
             context_len,  
             payload_len,  
             nonce,  
             content_hash,  
         }  
     }  
   
     /// Validate magic bytes and version  
     pub fn validate(&self) -> AdtpResult<()> {  
         if self.magic != ADTP_MAGIC {  
             return Err(AdtpError::InvalidMagic { got: self.magic });  
         }  
         if self.version != ADTP_VERSION {  
             return Err(AdtpError::UnsupportedVersion { version: self.version });  
         }  
         if self.payload_len > ADTP_MAX_FILE_SIZE {  
             return Err(AdtpError::PacketTooLarge {  
                 size: self.payload_len,  
                 max: ADTP_MAX_FILE_SIZE,  
             });  
         }  
         if self.context_len as usize > ADTP_MAX_CONTEXT_SIZE {  
             return Err(AdtpError::ContextTooLarge {  
                 size: self.context_len as usize,  
             });  
         }  
         Ok(())  
     }  
 }  
   
 /// The complete ADTP packet — header + context + payload + signature  
 #[derive(Debug, Clone)]  
 pub struct AdtpPacket {  
     /// Fixed-size header  
     pub header: PacketHeader,  
   
     /// Intelligence context (JSON, plaintext)  
     /// Contains FileMeta + ContextBlock  
     pub context: Bytes,  
   
     /// Encrypted file payload (AES-256-GCM)  
     pub payload: Bytes,  
   
     /// ed25519 signature (64 bytes)  
     /// Signs: bincode(header) + blake3(context) + blake3(payload)  
     pub signature: [u8; 64],  
 }  
   
 impl AdtpPacket {  
     /// Serialise the full packet to bytes for sending over QUIC  
     ///  
     /// Wire layout:  
     ///   [header: bincode serialised]  
     ///   [context_len bytes of context]  
     ///   [payload_len bytes of payload]  
     ///   [64 bytes of signature]  
     pub fn to_bytes(&self) -> AdtpResult<Bytes> {  
         let header_bytes = bincode::serialize(&self.header)  
             .map_err(AdtpError::Serialisation)?;  
   
         let total = header_bytes.len()  
             + self.context.len()  
             + self.payload.len()  
             + 64; // signature  
   
         let mut buf = BytesMut::with_capacity(total);  
   
         // Write header length prefix so receiver knows how many bytes to read  
         buf.put_u32(header_bytes.len() as u32);  
         buf.put_slice(&header_bytes);  
         buf.put_slice(&self.context);  
         buf.put_slice(&self.payload);  
         buf.put_slice(&self.signature);  
   
         Ok(buf.freeze())  
     }  
   
     /// Deserialise a packet from raw bytes received over QUIC  
     pub fn from_bytes(mut data: Bytes) -> AdtpResult<Self> {  
         // Read header length prefix  
         if data.remaining() < 4 {  
             return Err(AdtpError::MalformedPacket {  
                reason: "Too short to contain header length".to_string(),  
             });  
         }  
         let header_len = data.get_u32() as usize;  
   
         if data.remaining() < header_len {  
             return Err(AdtpError::MalformedPacket {  
                 reason: format!(  
                     "Expected {} header bytes, got {}",  
                     header_len,  
                     data.remaining()  
                 ),  
             });  
         }  
   
         let header_bytes = data.copy_to_bytes(header_len);  
         let header: PacketHeader = bincode::deserialize(&header_bytes)  
             .map_err(AdtpError::Serialisation)?;  
   
         header.validate()?;  
   
         let context = data.copy_to_bytes(header.context_len as usize);  
         let payload = data.copy_to_bytes(header.payload_len as usize);  
   
         if data.remaining() < 64 {  
             return Err(AdtpError::MalformedPacket {  
                 reason: "Missing signature".to_string(),  
             });  
         }  
         let mut signature = [0u8; 64];  
         data.copy_to_slice(&mut signature);  
   
         Ok(Self { header, context, payload, signature })  
     }  
   
     /// Get the bytes that were signed — used for signature verification  
     /// Signs: bincode(header) + blake3(context_bytes) + blake3(payload_bytes)  
     pub fn signable_bytes(&self) -> AdtpResult<Vec<u8>> {  
         let header_bytes = bincode::serialize(&self.header)  
             .map_err(AdtpError::Serialisation)?;  
   
         let context_hash = blake3::hash(&self.context);  
         let payload_hash = blake3::hash(&self.payload);  
   
         let mut signable = Vec::with_capacity(  
             header_bytes.len() + 32 + 32  
         );  
         signable.extend_from_slice(&header_bytes);  
         signable.extend_from_slice(context_hash.as_bytes());  
         signable.extend_from_slice(payload_hash.as_bytes());  
   
         Ok(signable)  
     }  
 }  
   
 /// Context envelope — what goes in the context block  
 #[derive(Debug, Clone, Serialize, Deserialize)]  
 pub struct ContextEnvelope {  
     pub file_meta: FileMeta,  
     pub context: ContextBlock,  
 }  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAAM0lEQVR4nO3OMQ0AIAwAwdIgBKl1gjacsGCAiZDcTT9+q6oRETMAAPjF6ify6QYAADdyA9Y0AypN+bdfAAAAAElFTkSuQmCC)  
**STEP 6 — BUILD **integrity.rs  
**File:** crates/adtp-core/src/integrity.rs  
BLAKE3 hashing and ed25519 signing. The trust layer of ADTP.  
use blake3::Hasher;  
 use ed25519_dalek::{  
     Signature, Signer, SigningKey, Verifier, VerifyingKey,  
 };  
 use rand::rngs::OsRng;  
   
 use crate::types::BLAKE3_HASH_SIZE;  
 use crate::errors::{AdtpError, AdtpResult};  
   
 // ─── BLAKE3 HASHING ───────────────────────────────────────────────────────────  
   
 /// Hash any byte slice with BLAKE3.  
 /// Returns 32-byte hash.  
 /// Use for: file content verification, signing material  
 pub fn hash_bytes(data: &[u8]) -> [u8; BLAKE3_HASH_SIZE] {  
     *blake3::hash(data).as_bytes()  
 }  
   
 /// Hash a file from disk using streaming BLAKE3.  
 /// Memory efficient — does NOT load entire file into RAM.  
 /// Use for large files (>100MB).  
 pub async fn hash_file(path: &std::path::Path) -> AdtpResult<[u8; BLAKE3_HASH_SIZE]> {  
     use tokio::io::AsyncReadExt;  
   
     let mut file = tokio::fs::File::open(path).await.map_err(|e| {  
         AdtpError::FileNotFound { path: path.to_string_lossy().to_string() }  
     })?;  
   
     let mut hasher = Hasher::new();  
     let mut buffer = vec![0u8; 64 * 1024]; // 64KB chunks  
   
     loop {  
         let n = file.read(&mut buffer).await?;  
         if n == 0 { break; }  
         hasher.update(&buffer[..n]);  
     }  
   
     Ok(*hasher.finalize().as_bytes())  
 }  
   
 /// Verify a BLAKE3 hash matches expected value.  
 /// Returns Ok(()) if match, Err if mismatch.  
 pub fn verify_hash(  
     data: &[u8],  
     expected: &[u8; BLAKE3_HASH_SIZE],  
 ) -> AdtpResult<()> {  
     let actual = hash_bytes(data);  
     if &actual != expected {  
         return Err(AdtpError::HashMismatch {  
             expected: hex::encode(expected),  
             got: hex::encode(actual),  
         });  
     }  
     Ok(())  
 }  
   
 /// Convert hash bytes to hex string (for logging/audit)  
 pub fn hash_to_hex(hash: &[u8; BLAKE3_HASH_SIZE]) -> String {  
     hex::encode(hash)  
 }  
   
 // ─── ED25519 SIGNING ──────────────────────────────────────────────────────────  
   
 /// Generate a fresh ed25519 keypair for a new session.  
 /// Call this once per session — never reuse keys.  
 pub fn generate_keypair() -> (SigningKey, VerifyingKey) {  
     let signing_key = SigningKey::generate(&mut OsRng);  
     let verifying_key = signing_key.verifying_key();  
     (signing_key, verifying_key)  
 }  
   
 /// Sign bytes with an ed25519 signing key.  
 /// Returns 64-byte signature.  
 pub fn sign(data: &[u8], signing_key: &SigningKey) -> [u8; 64] {  
     signing_key.sign(data).to_bytes()  
 }  
   
 /// Verify an ed25519 signature.  
 /// Returns Ok(()) if valid, Err if invalid.  
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
   
 // ─── AUDIT CHAIN ──────────────────────────────────────────────────────────────  
   
 /// Compute the audit chain hash for a new log entry.  
 /// Each entry's hash depends on the previous entry's hash —  
 /// creating a tamper-evident linked chain.  
 ///  
 /// chain_hash = BLAKE3(previous_hash || entry_data)  
 pub fn compute_chain_hash(  
     previous_hash: &[u8; BLAKE3_HASH_SIZE],  
     entry_data: &[u8],  
 ) -> [u8; BLAKE3_HASH_SIZE] {  
     let mut hasher = Hasher::new();  
     hasher.update(previous_hash);  
     hasher.update(entry_data);  
     *hasher.finalize().as_bytes()  
 }  
   
 /// Genesis hash — used as the previous_hash for the very first audit entry  
 pub fn genesis_hash() -> [u8; BLAKE3_HASH_SIZE] {  
     hash_bytes(b"ADTP_GENESIS_ASHIR_TECH_2026")  
 }  
   
***Note:*** * Add * *hex = "0.4"* * to your * *[workspace.dependencies]* * and adtp-core * *[dependencies]* *.*  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSfYxZo/jzlMYQLPJrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA4q7Bc870TqdAAAAAElFTkSuQmCC)  
**STEP 7 — BUILD **encryption.rs  
**File:** crates/adtp-core/src/encryption.rs  
AES-256-GCM encryption. Every file payload is encrypted before it leaves the machine.  
use aes_gcm::{  
     aead::{Aead, AeadCore, KeyInit, OsRng},  
     Aes256Gcm, Key, Nonce,  
 };  
 use hkdf::Hkdf;  
 use sha2::Sha256;  
 use rand::RngCore;  
   
 use crate::types::{AES_KEY_SIZE, AES_NONCE_SIZE};  
 use crate::errors::{AdtpError, AdtpResult};  
   
 // ─── KEY DERIVATION ───────────────────────────────────────────────────────────  
   
 /// Derive a symmetric AES-256 key from a shared secret using HKDF-SHA256.  
 ///  
 /// Call this after ECDH key exchange to get the session encryption key.  
 ///  
 /// # Arguments  
 /// * `shared_secret` — raw bytes from ECDH (X25519 output)  
 /// * `session_id`    — UUID bytes, used as HKDF salt for uniqueness  
 /// * `info`          — context string, use b"adtp-session-key"  
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
   
 // ─── NONCE GENERATION ─────────────────────────────────────────────────────────  
   
 /// Generate a cryptographically random 12-byte nonce.  
 /// NEVER reuse a nonce with the same key.  
 /// Generate fresh for every packet.  
 pub fn generate_nonce() -> [u8; AES_NONCE_SIZE] {  
     let mut nonce = [0u8; AES_NONCE_SIZE];  
     OsRng.fill_bytes(&mut nonce);  
     nonce  
 }  
   
 // ─── ENCRYPTION ───────────────────────────────────────────────────────────────  
   
 /// Encrypt plaintext bytes with AES-256-GCM.  
 ///  
 /// Returns ciphertext with AES-GCM authentication tag appended.  
 /// The tag (16 bytes) is automatically included by the aes-gcm crate.  
 ///  
 /// # Arguments  
 /// * `key`       — 32-byte AES-256 key  
 /// * `nonce`     — 12-byte nonce (generate fresh per packet)  
 /// * `plaintext` — raw file bytes to encrypt  
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
   
 /// Decrypt AES-256-GCM ciphertext.  
 ///  
 /// Verifies the authentication tag — if the data was tampered,  
 /// decryption will fail with DecryptionFailed error.  
 ///  
 /// # Arguments  
 /// * `key`        — 32-byte AES-256 key (must be same as used to encrypt)  
 /// * `nonce`      — 12-byte nonce (must be same as used to encrypt)  
 /// * `ciphertext` — encrypted bytes including appended auth tag  
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
   
 // ─── FILE ENCRYPTION ──────────────────────────────────────────────────────────  
   
 /// Encrypt a file's raw bytes for inclusion in an ADTP packet payload.  
 ///  
 /// Returns (ciphertext, nonce) — both must be stored in the packet header.  
 pub fn encrypt_file_bytes(  
     key: &[u8; AES_KEY_SIZE],  
     file_bytes: &[u8],  
 ) -> AdtpResult<(Vec<u8>, [u8; AES_NONCE_SIZE])> {  
     let nonce = generate_nonce();  
     let ciphertext = encrypt(key, &nonce, file_bytes)?;  
     Ok((ciphertext, nonce))  
 }  
   
 /// Decrypt a file payload from an ADTP packet.  
 ///  
 /// Returns raw file bytes ready to write to disk.  
 pub fn decrypt_file_bytes(  
     key: &[u8; AES_KEY_SIZE],  
     nonce: &[u8; AES_NONCE_SIZE],  
     ciphertext: &[u8],  
 ) -> AdtpResult<Vec<u8>> {  
     decrypt(key, nonce, ciphertext)  
 }  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AABAAsSNhYMMAKlD4OzrxgQU2QtIq6DIzR3UFAMBf3Gu1VefXEwAAXtsfSqADWz4G/HUAAAAASUVORK5CYII=)  
**STEP 8 — BUILD **session.rs  
**File:** crates/adtp-core/src/session.rs  
Session management — keypair generation and ECDH key exchange.  
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};  
 use ed25519_dalek::{SigningKey, VerifyingKey};  
 use rand::rngs::OsRng;  
 use uuid::Uuid;  
 use chrono::Utc;  
   
 use crate::types::{Session, AES_KEY_SIZE};  
 use crate::encryption::derive_session_key;  
 use crate::integrity::generate_keypair;  
 use crate::errors::{AdtpError, AdtpResult};  
   
 /// Full session keypair — one per connection  
 pub struct SessionKeys {  
     /// For ECDH key exchange (gives us the shared secret)  
     pub ecdh_secret: EphemeralSecret,  
     pub ecdh_public: PublicKey,  
   
     /// For signing packets  
     pub signing_key: SigningKey,  
     pub verifying_key: VerifyingKey,  
 }  
   
 impl SessionKeys {  
     /// Generate fresh keypairs for a new session.  
     /// Never reuse these — ephemeral by design.  
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
   
     /// Complete ECDH key exchange with remote peer's public key.  
     /// Returns the derived symmetric AES-256 session key.  
     ///  
     /// Call this after receiving the remote's public key during handshake.  
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
   
     /// Get the public ECDH key bytes to send to the remote peer  
     pub fn public_key_bytes(&self) -> [u8; 32] {  
         self.ecdh_public.to_bytes()  
     }  
   
     /// Get the verifying key bytes to send to the remote peer  
     pub fn verifying_key_bytes(&self) -> [u8; 32] {  
         self.verifying_key.to_bytes()  
     }  
 }  
   
 /// Create a new Session record  
 pub fn new_session(peer_addr: impl Into<String>) -> Session {  
     Session {  
         id: Uuid::new_v4(),  
         created_at: Utc::now(),  
         peer_addr: peer_addr.into(),  
         symmetric_key: None,  
     }  
 }  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANElEQVR4nO3OMQ0AIAwAwZIgBKn1gjJsdGLBABMhuZt+/JaZIyJmAADwi9VP1NMNAABu1AaU4gUeBSGW2wAAAABJRU5ErkJggg==)  
**STEP 9 — BUILD **transfer.rs  
**File:** crates/adtp-core/src/transfer.rs  
The QUIC transport engine. This is where ADTP meets the network.  
use quinn::{  
     ClientConfig, Endpoint, ServerConfig,  
     Connection, RecvStream, SendStream,  
 };  
 use rustls::pki_types::{CertificateDer, PrivateKeyDer};  
 use bytes::Bytes;  
 use tokio::io::{AsyncReadExt, AsyncWriteExt};  
 use std::net::SocketAddr;  
 use std::sync::Arc;  
   
 use crate::protocol::AdtpPacket;  
 use crate::errors::{AdtpError, AdtpResult};  
 use crate::types::ADTP_DEFAULT_PORT;  
   
 /// Size of the length prefix used when framing packets over QUIC streams  
 const LENGTH_PREFIX_SIZE: usize = 8; // u64 — supports packets up to 18 exabytes  
   
 // ─── SERVER ───────────────────────────────────────────────────────────────────  
   
 /// ADTP QUIC server — listens for incoming connections from clients  
 pub struct AdtpServer {  
     endpoint: Endpoint,  
 }  
   
 impl AdtpServer {  
     /// Start the ADTP server on the given address.  
     ///  
     /// For development: uses self-signed certificate (generated at startup).  
     /// For production: load real TLS cert + key from disk.  
     pub async fn bind(addr: SocketAddr) -> AdtpResult<Self> {  
         let (cert, key) = generate_self_signed_cert()?;  
         let server_config = build_server_config(cert, key)?;  
   
         let endpoint = Endpoint::server(server_config, addr)  
             .map_err(|e| AdtpError::BindFailed {  
                 addr: addr.to_string(),  
                 reason: e.to_string(),  
             })?;  
   
         tracing::info!("ADTP server listening on {}", addr);  
   
         Ok(Self { endpoint })  
     }  
   
     /// Accept incoming QUIC connections.  
     /// Returns a connected AdtpConnection for each accepted client.  
     pub async fn accept(&self) -> AdtpResult<Option<AdtpConnection>> {  
         match self.endpoint.accept().await {  
             Some(incoming) => {  
                 let connection = incoming  
                     .await  
                     .map_err(|e| AdtpError::QuicConnectionFailed {  
                         reason: e.to_string(),  
                     })?;  
   
                 let peer = connection.remote_address().to_string();  
                 tracing::info!("New ADTP connection from {}", peer);  
   
                 Ok(Some(AdtpConnection::new(connection)))  
             }  
             None => Ok(None),  
         }  
     }  
 }  
   
 // ─── CLIENT ───────────────────────────────────────────────────────────────────  
   
 /// ADTP QUIC client — connects to a server  
 pub struct AdtpClient {  
     endpoint: Endpoint,  
 }  
   
 impl AdtpClient {  
     /// Create a new ADTP client endpoint.  
     pub async fn new() -> AdtpResult<Self> {  
         // Development: skip TLS verification (self-signed certs)  
         // Production: load CA cert and verify properly  
         let client_config = build_client_config_insecure();  
   
         let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())  
             .map_err(|e| AdtpError::QuicConnectionFailed {  
                 reason: e.to_string(),  
             })?;  
   
         endpoint.set_default_client_config(client_config);  
   
         Ok(Self { endpoint })  
     }  
   
     /// Connect to an ADTP server.  
     pub async fn connect(&self, addr: SocketAddr, server_name: &str) -> AdtpResult<AdtpConnection> {  
         let connection = self.endpoint  
             .connect(addr, server_name)  
             .map_err(|e| AdtpError::QuicConnectionFailed {  
                 reason: e.to_string(),  
             })?  
             .await  
             .map_err(|e| AdtpError::QuicConnectionFailed {  
                 reason: e.to_string(),  
             })?;  
   
         tracing::info!("Connected to ADTP server at {}", addr);  
   
         Ok(AdtpConnection::new(connection))  
     }  
 }  
   
 // ─── CONNECTION ───────────────────────────────────────────────────────────────  
   
 /// An active ADTP QUIC connection — either client or server side.  
 pub struct AdtpConnection {  
     inner: Connection,  
 }  
   
 impl AdtpConnection {  
     pub fn new(connection: Connection) -> Self {  
         Self { inner: connection }  
     }  
   
     /// Send an ADTP packet over a new QUIC stream.  
     ///  
     /// Each packet gets its own QUIC stream — enabling multiplexing.  
     /// Multiple packets can be in flight simultaneously with no head-of-line blocking.  
     pub async fn send_packet(&self, packet: &AdtpPacket) -> AdtpResult<()> {  
         let (mut send, _recv) = self.inner  
             .open_bi()  
             .await  
             .map_err(|e| AdtpError::QuicStreamError {  
                 reason: e.to_string(),  
             })?;  
   
         let packet_bytes = packet.to_bytes()?;  
         write_framed(&mut send, &packet_bytes).await?;  
         send.finish().map_err(|e| AdtpError::QuicStreamError {  
             reason: e.to_string(),  
         })?;  
   
         Ok(())  
     }  
   
     /// Receive an ADTP packet from an incoming QUIC stream.  
     pub async fn recv_packet(&self) -> AdtpResult<AdtpPacket> {  
         let (_, mut recv) = self.inner  
             .accept_bi()  
             .await  
             .map_err(|e| AdtpError::QuicStreamError {  
                 reason: e.to_string(),  
             })?;  
   
         let packet_bytes = read_framed(&mut recv).await?;  
         AdtpPacket::from_bytes(Bytes::from(packet_bytes))  
     }  
   
     /// Remote peer address  
     pub fn peer_addr(&self) -> String {  
         self.inner.remote_address().to_string()  
     }  
   
     /// Close the connection gracefully  
     pub async fn close(&self) {  
         self.inner.close(0u32.into(), b"goodbye");  
     }  
 }  
   
 // ─── FRAMING ──────────────────────────────────────────────────────────────────  
 // QUIC streams are byte streams with no built-in message boundaries.  
 // We prefix every message with its length (8 bytes, u64, big-endian)  
 // so the receiver knows how many bytes to read.  
   
 async fn write_framed(send: &mut SendStream, data: &[u8]) -> AdtpResult<()> {  
     let len = data.len() as u64;  
     send.write_all(&len.to_be_bytes()).await?;  
     send.write_all(data).await?;  
     Ok(())  
 }  
   
 async fn read_framed(recv: &mut RecvStream) -> AdtpResult<Vec<u8>> {  
     let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];  
     recv.read_exact(&mut len_buf).await  
         .map_err(|e| AdtpError::QuicStreamError { reason: e.to_string() })?;  
   
     let len = u64::from_be_bytes(len_buf) as usize;  
   
     let mut data = vec![0u8; len];  
     recv.read_exact(&mut data).await  
         .map_err(|e| AdtpError::QuicStreamError { reason: e.to_string() })?;  
   
     Ok(data)  
 }  
   
 // ─── TLS HELPERS ─────────────────────────────────────────────────────────────  
   
 /// Generate a self-signed TLS certificate for development.  
 /// Replace with real cert in production.  
 fn generate_self_signed_cert() -> AdtpResult<(CertificateDer<'static>, PrivateKeyDer<'static>)> {  
     let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])  
         .map_err(|e| AdtpError::TlsError { reason: e.to_string() })?;  
   
     let cert_der = CertificateDer::from(cert.cert.der().to_vec());  
     let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der())  
         .map_err(|e| AdtpError::TlsError { reason: e.to_string() })?;  
   
     Ok((cert_der, key_der))  
 }  
   
 fn build_server_config(  
     cert: CertificateDer<'static>,  
     key: PrivateKeyDer<'static>,  
 ) -> AdtpResult<ServerConfig> {  
     let mut server_config = ServerConfig::with_single_cert(vec![cert], key)  
         .map_err(|e| AdtpError::TlsError { reason: e.to_string() })?;  
   
     let transport = Arc::get_mut(&mut server_config.transport).unwrap();  
     transport.max_concurrent_bidi_streams(1000u32.into());  
   
     Ok(server_config)  
 }  
   
 fn build_client_config_insecure() -> ClientConfig {  
     // Development only — skips TLS cert verification  
     // TODO: Replace with proper CA verification for production  
     let crypto = rustls::ClientConfig::builder()  
         .dangerous()  
         .with_custom_certificate_verifier(Arc::new(SkipCertVerification))  
         .with_no_client_auth();  
   
     ClientConfig::new(Arc::new(  
         quinn::crypto::rustls::QuicClientConfig::try_from(crypto).unwrap()  
     ))  
 }  
   
 /// Development-only TLS verifier — accepts any certificate  
 struct SkipCertVerification;  
   
 impl rustls::client::danger::ServerCertVerifier for SkipCertVerification {  
     fn verify_server_cert(  
         &self, _: &CertificateDer, _: &[CertificateDer],  
         _: &rustls::pki_types::ServerName, _: &[u8], _: rustls::pki_types::UnixTime,  
     ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {  
         Ok(rustls::client::danger::ServerCertVerified::assertion())  
     }  
   
     fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {  
         Ok(rustls::client::danger::HandshakeSignatureValid::assertion())  
     }  
   
     fn verify_tls13_signature(&self, _: &[u8], _: &CertificateDer, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {  
         Ok(rustls::client::danger::HandshakeSignatureValid::assertion())  
     }  
   
     fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {  
         vec![  
             rustls::SignatureScheme::ED25519,  
             rustls::SignatureScheme::ECDSA_NISTP256_SHA256,  
             rustls::SignatureScheme::RSA_PSS_SHA256,  
         ]  
     }  
 }  
   
***Add to workspace dependencies:*** * * *rcgen = "0.13"* * and * *quinn = { version = "0.11", features = ["rustls"] }*  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSfYxZo/jkUsYQLPJrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA4rDBc72meO5AAAAAElFTkSuQmCC)  
**STEP 10 — BUILD **lib.rs  
**File:** crates/adtp-core/src/lib.rs  
The public interface of adtp-core — what other crates import.  
//! # adtp-core  
 //!  
 //! The foundational layer of ADTP — Agent Direct Transfer Protocol.  
 //!  
 //! ## What this crate provides  
 //! - `types`      — All shared types (packets, sessions, transfers)  
 //! - `errors`     — All error types  
 //! - `protocol`   — ADTP packet structure and serialisation  
 //! - `integrity`  — BLAKE3 hashing and ed25519 signing  
 //! - `encryption` — AES-256-GCM encryption/decryption  
 //! - `session`    — Session and keypair management  
 //! - `transfer`   — QUIC transport engine  
 //!  
 //! ## Example  
 //! ```rust  
 //! use adtp_core::types::*;  
 //! use adtp_core::integrity::hash_bytes;  
 //! use adtp_core::encryption::{encrypt_file_bytes, decrypt_file_bytes};  
 //!  
 //! let key = [0u8; 32]; // In real code: derive from session  
 //! let file_data = b"Hello from Ashir Tech";  
 //! let (ciphertext, nonce) = encrypt_file_bytes(&key, file_data).unwrap();  
 //! let decrypted = decrypt_file_bytes(&key, &nonce, &ciphertext).unwrap();  
 //! assert_eq!(file_data, decrypted.as_slice());  
 //! ```  
   
 pub mod types;  
 pub mod errors;  
 pub mod protocol;  
 pub mod integrity;  
 pub mod encryption;  
 pub mod session;  
 pub mod transfer;  
   
 // Re-export most commonly used items  
 pub use types::*;  
 pub use errors::{AdtpError, AdtpResult};  
 pub use protocol::{AdtpPacket, PacketHeader, ContextEnvelope};  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSfYxZo/jVEMYQLPJrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA4rLBc059ysnAAAAAElFTkSuQmCC)  
**STEP 11 — UNIT TESTS**  
Add these tests at the bottom of each file inside a #[cfg(test)] module. Run with cargo test.  
**Test: **integrity.rs  
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
         // Chain hashes must be different and deterministic  
         assert_ne!(hash1, hash2);  
         assert_eq!(hash2, compute_chain_hash(&hash1, entry2));  
     }  
 }  
   
**Test: **encryption.rs  
#[cfg(test)]  
 mod tests {  
     use super::*;  
   
     #[test]  
     fn test_encrypt_decrypt_roundtrip() {  
         let key = [42u8; 32];  
         let plaintext = b"Hello Ashir Tech — ADTP protocol test";  
         let (ciphertext, nonce) = encrypt_file_bytes(&key, plaintext).unwrap();  
         assert_ne!(&ciphertext, plaintext); // must be different  
         let decrypted = decrypt_file_bytes(&key, &nonce, &ciphertext).unwrap();  
         assert_eq!(decrypted, plaintext);  
     }  
   
     #[test]  
     fn test_wrong_key_fails() {  
         let key = [1u8; 32];  
         let wrong_key = [2u8; 32];  
         let plaintext = b"secret data";  
         let (ciphertext, nonce) = encrypt_file_bytes(&key, plaintext).unwrap();  
         assert!(decrypt_file_bytes(&wrong_key, &nonce, &ciphertext).is_err());  
     }  
   
     #[test]  
     fn test_tampered_ciphertext_fails() {  
         let key = [7u8; 32];  
         let plaintext = b"important file";  
         let (mut ciphertext, nonce) = encrypt_file_bytes(&key, plaintext).unwrap();  
         ciphertext[0] ^= 0xFF; // flip bits — tamper the ciphertext  
         assert!(decrypt_file_bytes(&key, &nonce, &ciphertext).is_err());  
     }  
   
     #[test]  
     fn test_nonce_uniqueness() {  
         let n1 = generate_nonce();  
         let n2 = generate_nonce();  
         assert_ne!(n1, n2); // must never repeat  
     }  
   
     #[test]  
     fn test_key_derivation() {  
         let secret = [0u8; 32];  
         let session_id = [1u8; 16];  
         let key1 = derive_session_key(&secret, &session_id, b"adtp-session-key").unwrap();  
         let key2 = derive_session_key(&secret, &session_id, b"adtp-session-key").unwrap();  
         assert_eq!(key1, key2); // deterministic  
     }  
 }  
   
**Test: **protocol.rs  
#[cfg(test)]  
 mod tests {  
     use super::*;  
   
     fn make_test_packet() -> AdtpPacket {  
         let session_id = Uuid::new_v4();  
         let transfer_id = Uuid::new_v4();  
         let nonce = [0u8; 12];  
         let content_hash = [0u8; 32];  
         let context = Bytes::from(b"{}".to_vec());  
         let payload = Bytes::from(b"encrypted bytes here".to_vec());  
   
         let header = PacketHeader::new(  
             TransferType::Send,  
             session_id,  
             transfer_id,  
             nonce,  
            content_hash,  
             context.len() as u32,  
             payload.len() as u64,  
         );  
   
         AdtpPacket {  
             header,  
             context,  
             payload,  
             signature: [0u8; 64],  
         }  
     }  
   
     #[test]  
     fn test_packet_serialise_deserialise() {  
         let packet = make_test_packet();  
         let bytes = packet.to_bytes().unwrap();  
         let decoded = AdtpPacket::from_bytes(bytes).unwrap();  
         assert_eq!(decoded.header.magic, ADTP_MAGIC);  
         assert_eq!(decoded.header.version, ADTP_VERSION);  
         assert_eq!(decoded.payload, packet.payload);  
     }  
   
     #[test]  
     fn test_invalid_magic_rejected() {  
         let mut packet = make_test_packet();  
         packet.header.magic = 0xDEADBEEF;  
         // Manually serialise with wrong magic  
         // This should fail validation  
         // (implement via from_bytes returning Err)  
         let bytes = packet.to_bytes().unwrap();  
         let result = AdtpPacket::from_bytes(bytes);  
         assert!(result.is_err());  
     }  
   
     #[test]  
     fn test_signable_bytes_deterministic() {  
         let packet = make_test_packet();  
         let sig1 = packet.signable_bytes().unwrap();  
         let sig2 = packet.signable_bytes().unwrap();  
         assert_eq!(sig1, sig2);  
     }  
 }  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANElEQVR4nO3OQQmAUBBAwSd8bOHVnBvBkAaxgjcRZhLMNjNHdQUAwF/cq9qr8+sJAACvrQctgQNH4A++9QAAAABJRU5ErkJggg==)  
**STEP 12 — BUILD AND VERIFY**  
Run these commands to confirm Phase 0 compiles clean:  
# From repo root  
 cd adtp  
   
 # Check everything compiles  
 cargo check --workspace  
   
 # Run all tests  
 cargo test --package adtp-core  
   
 # Check for warnings and style issues  
 cargo clippy --package adtp-core -- -D warnings  
   
 # Format code  
 cargo fmt --package adtp-core  
   
 # Check dependency vulnerabilities  
 cargo audit  
   
**Expected output:**  
running 14 tests  
 test encryption::tests::test_encrypt_decrypt_roundtrip ... ok  
 test encryption::tests::test_wrong_key_fails ... ok  
 test encryption::tests::test_tampered_ciphertext_fails ... ok  
 test encryption::tests::test_nonce_uniqueness ... ok  
 test encryption::tests::test_key_derivation ... ok  
 test integrity::tests::test_hash_bytes_deterministic ... ok  
 test integrity::tests::test_hash_verify_pass ... ok  
 test integrity::tests::test_hash_verify_tampered ... ok  
 test integrity::tests::test_sign_and_verify ... ok  
 test integrity::tests::test_verify_wrong_key_fails ... ok  
 test integrity::tests::test_audit_chain ... ok  
 test protocol::tests::test_packet_serialise_deserialise ... ok  
 test protocol::tests::test_invalid_magic_rejected ... ok  
 test protocol::tests::test_signable_bytes_deterministic ... ok  
   
 test result: ok. 14 passed; 0 failed  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AUBBAsUfyNTCi9VwgEA3sWGAjJK2CbjNzVGcAAPzFtapV7V9PAAB47X4AEW4ELQDBN+AAAAAASUVORK5CYII=)  
**PHASE 0 COMPLETION CHECKLIST**  
Before moving to Phase 1, every item must be checked:  
□ Cargo workspace builds with zero errors  
 □ All 14 unit tests pass  
 □ cargo clippy returns zero warnings  
 □ cargo fmt applied to all files  
 □ types.rs — all shared types defined  
 □ errors.rs — all error variants defined  
 □ protocol.rs — packet serialises and deserialises correctly  
 □ integrity.rs — BLAKE3 hash and ed25519 signing working  
 □ encryption.rs — AES-256-GCM encrypt/decrypt roundtrip passing  
 □ session.rs — keypair generation working  
 □ transfer.rs — QUIC server and client compile without errors  
 □ lib.rs — all modules re-exported cleanly  
 *.*  
