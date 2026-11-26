//! Canonical, serde-compatible `MediaRef` used by every ADTP client.
//!
//! Field names intentionally match the shared MARS wire format (camelCase).
//! v1 payloads (which only had `compressedSize`) deserialize through an alias
//! and are upgraded by [`MediaRef::normalize`].

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::validation::validate_media_ref;

fn default_mime() -> String {
    "application/octet-stream".into()
}

/// A stored media object. Only this metadata crosses chat/device transports.
#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaRef {
    #[serde(default = "default_schema_version")]
    pub schema_version: u8,
    pub id: String,
    pub service_url: String,
    pub name: String,
    #[serde(default = "default_mime")]
    pub mime_type: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default, alias = "compressedSize")]
    pub stored_size: i64,
    #[serde(default)]
    pub width: Option<i64>,
    #[serde(default)]
    pub height: Option<i64>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub recipient: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub compressed: bool,
    #[serde(default)]
    pub has_thumbnail: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub expires_at: String,
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_schema_version() -> u8 {
    1
}

fn default_status() -> String {
    "ready".into()
}

impl MediaRef {
    /// Deserialize untrusted JSON, upgrade v1 fields and validate.
    pub fn normalize(value: &Value) -> Option<Self> {
        let mut media: Self = serde_json::from_value(value.clone()).ok()?;
        if media.stored_size == 0 {
            if let Some(legacy) = value.get("compressedSize").and_then(Value::as_i64) {
                media.stored_size = legacy;
            }
        }
        if value.get("schemaVersion").is_none() {
            media.schema_version = 1;
        }
        // The deployed MARS relay uses `pending` / `complete`.
        match media.status.as_str() {
            "pending" => media.status = "created".into(),
            "complete" | "completed" => media.status = "ready".into(),
            _ => {}
        }
        validate_media_ref(&media).ok()?;
        Some(media)
    }

    /// Deprecated v1 alias for the stored byte count.
    pub fn compressed_size(&self) -> i64 {
        self.stored_size
    }

    /// True once the retention deadline has passed or the object was deleted.
    pub fn is_expired(&self) -> bool {
        if matches!(self.status.as_str(), "expired" | "deleted") {
            return true;
        }
        if self.expires_at.is_empty() {
            return false;
        }
        match chrono::DateTime::parse_from_rfc3339(&self.expires_at) {
            Ok(deadline) => deadline.timestamp() <= chrono::Utc::now().timestamp(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v1() -> Value {
        serde_json::json!({
            "id": "media-1",
            "serviceUrl": "https://relay.example",
            "name": "photo.png",
            "mimeType": "image/png",
            "size": 9000,
            "compressedSize": 3000,
            "compressed": true,
            "hasThumbnail": true,
            "createdAt": "2026-11-20T00:00:00Z"
        })
    }

    #[test]
    fn v1_wire_shape_is_upgraded() {
        let media = MediaRef::normalize(&v1()).expect("v1 ref must parse");
        assert_eq!(media.schema_version, 1);
        assert_eq!(media.stored_size, 3000);
        assert_eq!(media.compressed_size(), 3000);
        assert_eq!(media.status, "ready");
        assert_eq!(media.session_id, None);
    }

    #[test]
    fn v2_wire_shape_round_trips_as_camel_case() {
        let value = serde_json::json!({
            "schemaVersion": 2,
            "id": "media-2",
            "serviceUrl": "https://relay.example",
            "name": "clip.jpg",
            "storedSize": 1234,
            "size": 9000,
            "expiresAt": "2999-01-01T00:00:00Z",
            "status": "ready",
            "conversationId": "default-session",
            "hasThumbnail": true
        });
        let media = MediaRef::normalize(&value).expect("v2 ref must parse");
        assert_eq!(media.schema_version, 2);
        assert_eq!(media.conversation_id.as_deref(), Some("default-session"));
        let json = serde_json::to_value(&media).unwrap();
        assert_eq!(json["schemaVersion"], 2);
        assert_eq!(json["storedSize"], 1234);
        assert_eq!(json["hasThumbnail"], true);
    }

    #[test]
    fn unsafe_or_invalid_refs_are_rejected() {
        let mut bad = v1();
        bad["serviceUrl"] = serde_json::json!("javascript:alert(1)");
        assert!(MediaRef::normalize(&bad).is_none());
        let mut bad = v1();
        bad["serviceUrl"] = serde_json::json!("https://user:pass@relay.example");
        assert!(MediaRef::normalize(&bad).is_none());
        let mut bad = v1();
        bad["size"] = serde_json::json!(-1);
        assert!(MediaRef::normalize(&bad).is_none());
    }

    #[test]
    fn expired_refs_are_detected() {
        let mut media = v1();
        media["expiresAt"] = serde_json::json!("2020-01-01T00:00:00Z");
        let media = MediaRef::normalize(&media).unwrap();
        assert!(media.is_expired());
        let mut media = v1();
        media["status"] = serde_json::json!("deleted");
        assert!(MediaRef::normalize(&media).unwrap().is_expired());
    }
}
