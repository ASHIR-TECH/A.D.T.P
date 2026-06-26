use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdtpError {
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
    #[error("Integrity check failed: hash mismatch. Expected {expected}, got {got}")]
    HashMismatch { expected: String, got: String },
    #[error("Signature verification failed")]
    SignatureInvalid,
    #[error("File tampered: content hash does not match header")]
    FileTampered,
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
    #[error("Session not found: {session_id}")]
    SessionNotFound { session_id: String },
    #[error("Session expired")]
    SessionExpired,
    #[error("Handshake failed: {reason}")]
    HandshakeFailed { reason: String },
    #[error("Authentication failed: invalid key")]
    AuthenticationFailed,
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
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialisation error: {0}")]
    Serialisation(#[from] bincode::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Context extraction failed for {mime_type}: {reason}")]
    ContextExtractionFailed { mime_type: String, reason: String },
    #[error("Context block too large: {size} bytes")]
    ContextTooLarge { size: usize },
    #[error("Folder not found: {path}")]
    FolderNotFound { path: String },
    #[error("Watch already registered for: {path}")]
    WatchAlreadyRegistered { path: String },
    #[error("Watch not found: {watch_id}")]
    WatchNotFound { watch_id: String },
    #[error("Internal error: {reason}")]
    Internal { reason: String },
    #[error("Task not found: {task_id}")]
    TaskNotFound { task_id: String },
    #[error("Illegal task state transition: {from} → {to} — {reason}")]
    IllegalTaskTransition { from: String, to: String, reason: String },
    #[error("Task is in terminal state: {state}")]
    TaskTerminal { state: String },
    #[error("Artifact not found for task {task_id}: {artifact_name}")]
    ArtifactNotFound {
        task_id: String,
        artifact_name: String,
    },
    #[error("Relay loop detected: peer {peer_id} already in relay_path")]
    RelayLoopDetected { peer_id: String },
    #[error("Relay hop limit exceeded: max {max} hops")]
    RelayHopLimitExceeded { max: u8 },
    #[error("Relay forwarding failed: {reason}")]
    RelayForwardFailed { reason: String },
    #[error("No route to peer {peer_id} via any relay")]
    NoRelayRoute { peer_id: String },
    #[error("Agent card verification failed: {reason}")]
    AgentCardVerificationFailed { reason: String },
    #[error("NAT detection failed: {reason}")]
    NatDetectionFailed { reason: String },
    #[error("Rendezvous error: {reason}")]
    RendezvousError { reason: String },
    #[error("Rendezvous token expired")]
    RendezvousTokenExpired,
    #[error("Rendezvous token replay rejected")]
    RendezvousTokenReplay,
    #[error("Hole punch failed: {reason}")]
    HolePunchFailed { reason: String },
    #[error("mDNS discovery failed: {reason}")]
    MdnsDiscoveryFailed { reason: String },
    #[error("QoWT connection failed: {reason}")]
    QowtConnectionFailed { reason: String },
    #[error("Keepalive timeout — peer {peer_id} unreachable")]
    KeepaliveTimeout { peer_id: String },
    #[error("All connection paths exhausted for peer {peer_id}")]
    AllPathsExhausted { peer_id: String },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type AdtpResult<T> = Result<T, AdtpError>;
