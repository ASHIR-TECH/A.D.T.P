use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ADTP_MAGIC: u32 = 0xAD7F1A00;
pub const ADTP_VERSION: u8 = 1;
pub const ADTP_DEFAULT_PORT: u16 = 40001;
pub const ADTP_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024 * 1024;
pub const ADTP_MAX_CONTEXT_SIZE: usize = 1024 * 1024;
pub const AES_KEY_SIZE: usize = 32;
pub const AES_NONCE_SIZE: usize = 12;
pub const AES_TAG_SIZE: usize = 16;
pub const ED25519_SIG_SIZE: usize = 64;
pub const BLAKE3_HASH_SIZE: usize = 32;
pub const PEER_ID_SIZE: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransferType {
    Transfer = 0x01,
    Ack = 0x03,
    Error = 0x04,
    Watch = 0x05,
    Unwatch = 0x06,
    Status = 0x07,
    History = 0x08,
    Hello = 0x09,
    Ping = 0x0A,
    Pong = 0x0B,
    Subscribe = 0x0C,
    Notify = 0x0D,
    TaskRequest = 0x0E,
    TaskUpdate = 0x0F,
    ArtifactReady = 0x10,
    PeerRegister = 0x20,
    RendezvousRequest = 0x21,
    RendezvousResponse = 0x22,
    PunchReady = 0x23,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Priority {
    Low = 0x00,
    #[default]
    Normal = 0x01,
    Urgent = 0x02,
    Critical = 0x03,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    Pending,
    InProgress { bytes_sent: u64, total_bytes: u64 },
    Delivered,
    Failed { reason: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub filename: String,
    pub mime_type: String,
    pub file_size: u64,
    pub extension: String,
    pub last_modified: Option<DateTime<Utc>>,
    pub content_hash: [u8; BLAKE3_HASH_SIZE],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextBlock {
    pub schema: Option<serde_json::Value>,
    pub summary: Option<String>,
    pub agent_hint: Option<String>,
    pub transfer_history: Vec<TransferHistoryEntry>,
    pub custom_metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferHistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub agent_action: Option<String>,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub peer_addr: String,
    #[serde(skip)]
    pub symmetric_key: Option<[u8; AES_KEY_SIZE]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerIdentity {
    pub peer_id: [u8; PEER_ID_SIZE],
    pub public_key: [u8; 32],
    pub adtp_version: u8,
    pub max_file_size: u64,
    pub supported_mime_types: Vec<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    Trusted,
    Unverified,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEntry {
    pub peer_id: String,
    pub public_key: String,
    pub nickname: Option<String>,
    pub host: String,
    pub port: u16,
    pub trust_level: TrustLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub sender_peer_id: [u8; PEER_ID_SIZE],
    pub receiver_peer_id: [u8; PEER_ID_SIZE],
    pub transfer_type: TransferType,
    pub file_meta: FileMeta,
    pub status: TransferStatus,
    pub initiated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub signature: String,
    pub received_hash: Option<[u8; BLAKE3_HASH_SIZE]>,
    pub integrity_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEntry {
    pub id: Uuid,
    pub folder_path: String,
    pub file_pattern: Option<String>,
    pub trigger_action: Option<String>,
    pub created_at: DateTime<Utc>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdtpAddr {
    pub host: String,
    pub port: u16,
}

impl AdtpAddr {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceId(pub [u8; BLAKE3_HASH_SIZE]);

impl TraceId {
    pub fn new() -> Self {
        let mut bytes = [0u8; BLAKE3_HASH_SIZE];
        use blake3;
        let hash = blake3::hash(uuid::Uuid::new_v4().as_bytes());
        bytes.copy_from_slice(hash.as_bytes());
        Self(bytes)
    }

    pub fn from_bytes(bytes: [u8; BLAKE3_HASH_SIZE]) -> Self {
        Self(bytes)
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayHop {
    pub peer_id: [u8; PEER_ID_SIZE],
    pub host: String,
    pub port: u16,
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayPath {
    pub hops: Vec<RelayHop>,
    pub hop_limit: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NatType {
    Open,
    FullCone,
    PortRestricted,
    Symmetric,
    Cgnat,
    IPv6,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRegisterPayload {
    pub peer_id: [u8; PEER_ID_SIZE],
    pub external_addr: String,
    pub nat_type: NatType,
    pub supports_ipv6: bool,
    pub can_relay: bool,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendezvousRequestPayload {
    pub requester_peer_id: [u8; PEER_ID_SIZE],
    pub target_peer_id: [u8; PEER_ID_SIZE],
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendezvousResponsePayload {
    pub token: String,
    pub target_external_addr: String,
    pub target_peer_id: [u8; PEER_ID_SIZE],
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PunchReadyPayload {
    pub token: String,
    pub sender_peer_id: [u8; PEER_ID_SIZE],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub rendezvous_server: Option<String>,
    pub rendezvous_port: u16,
    pub lan_discovery: bool,
    pub keepalive_interval_ms: u64,
    pub nat_type_override: Option<NatType>,
    pub stun_servers: Vec<String>,
    pub qowt_enabled: bool,
    pub qowt_gateway: Option<String>,
    pub connection_timeout_ms: u64,
    pub relay_enabled: bool,
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            rendezvous_server: None,
            rendezvous_port: 40004,
            lan_discovery: true,
            keepalive_interval_ms: 25000,
            nat_type_override: None,
            stun_servers: vec![
                "stun1.l.google.com:19302".into(),
                "stun2.l.google.com:19302".into(),
            ],
            qowt_enabled: true,
            qowt_gateway: None,
            connection_timeout_ms: 3000,
            relay_enabled: true,
        }
    }
}

pub const RENDEZVOUS_DEFAULT_PORT: u16 = 40004;
pub const KEEPALIVE_DEFAULT_MS: u64 = 25000;
pub const CONNECTION_TIMEOUT_DEFAULT_MS: u64 = 3000;
pub const ADTP_MDNS_SERVICE: &str = "_adtp._udp.local";
pub const TOKEN_VALIDITY_SECS: u64 = 30;
pub const PEER_TABLE_PURGE_SECS: u64 = 60;
pub const DEAD_DETECTION_PINGS: u8 = 3;
pub const RECONNECT_BACKOFF_MAX_SECS: u64 = 60;
