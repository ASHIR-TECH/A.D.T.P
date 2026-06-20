use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub daemon_connected: bool,
    pub audit_db_ok: bool,
    pub bridge_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueResponse {
    pub pending: i64,
    pub in_progress: i64,
    pub failed_last_hour: i64,
    pub avg_delivery_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSendResponse {
    pub transfer_id: String,
    pub status: String,
    pub filename: String,
    pub queued_at: Option<String>,
    pub estimated_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferStatus {
    pub status: String,
    pub bytes_sent: i64,
    pub total_bytes: i64,
    pub percent: f64,
    pub elapsed_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRecord {
    pub id: String,
    pub session_id: String,
    pub transfer_type: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: i64,
    pub status: String,
    pub integrity_verified: bool,
    pub signature: String,
    pub chain_hash: String,
    pub initiated_at: String,
    pub completed_at: Option<String>,
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferList {
    pub transfers: Vec<TransferRecord>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub session_id: String,
    pub transfer_type: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: i64,
    pub status: String,
    pub integrity_verified: bool,
    pub signature: String,
    pub chain_hash: String,
    pub initiated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogList {
    pub entries: Vec<AuditEntry>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainVerifyResult {
    pub valid: bool,
    pub entries_checked: i64,
    pub first_failure_id: Option<String>,
    pub verified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub peer_addr: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub transfer_count: i64,
    pub bytes_transferred: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionList {
    pub sessions: Vec<SessionInfo>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchResponse {
    pub watch_id: String,
    pub folder_path: String,
    pub file_pattern: Option<String>,
    pub trigger_action: Option<String>,
    pub active: bool,
    pub agent_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchList {
    pub watchers: Vec<WatchResponse>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBlock {
    #[serde(rename = "schema")]
    pub schema_: Option<serde_json::Value>,
    pub summary: Option<String>,
    pub agent_hint: Option<String>,
    pub transfer_history: Vec<ContextHistoryEntry>,
    pub extraction_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextHistoryEntry {
    pub timestamp: String,
    pub mime_type: String,
    pub agent_action: String,
}
