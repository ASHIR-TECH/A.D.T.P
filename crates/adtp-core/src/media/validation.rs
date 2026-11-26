//! Validation helpers for untrusted media metadata.

use super::types::MediaRef;

/// Statuses accepted on the wire. The deployed MARS relay uses the legacy
/// `pending` / `complete` values; `MediaRef::normalize` maps them here.
pub const MEDIA_STATUSES: &[&str] = &[
    "created",
    "uploading",
    "verifying",
    "ready",
    "failed",
    "cancelled",
    "deleting",
    "expired",
    "deleted",
];

/// Reject non-http(s) URLs, URLs without a host and embedded credentials.
/// Signed URLs are never part of this contract, but a forged `serviceUrl`
/// would otherwise turn the desktop WebView into a confused deputy.
pub fn validate_service_url(url: &str) -> Result<(), String> {
    let lower = url.trim().to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err("serviceUrl must be an http(s) URL".into());
    }
    let authority = url
        .split_once("://")
        .map(|(_, remainder)| remainder)
        .unwrap_or("")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    if authority.is_empty() {
        return Err("serviceUrl must have a host".into());
    }
    if authority.contains('@') {
        return Err("serviceUrl must not embed credentials".into());
    }
    Ok(())
}

/// Validate the whole reference at a trust boundary.
pub fn validate_media_ref(media: &MediaRef) -> Result<(), String> {
    if media.id.is_empty() || media.id.len() > 255 {
        return Err("media id must be a non-empty string of at most 255 chars".into());
    }
    if media.name.is_empty() || media.name.len() > 255 {
        return Err("media name must be a non-empty string of at most 255 chars".into());
    }
    for value in [&media.id, &media.name] {
        if value
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control())
        {
            return Err("media ids and names must not contain separators or control chars".into());
        }
    }
    validate_service_url(&media.service_url)?;
    if media.size < 0 || media.stored_size < 0 {
        return Err("media sizes must be non-negative".into());
    }
    if !media.mime_type.contains('/') || media.mime_type.len() > 150 {
        return Err("invalid media MIME type".into());
    }
    if let Some(checksum) = &media.sha256 {
        let valid =
            (32..=128).contains(&checksum.len()) && checksum.bytes().all(|b| b.is_ascii_hexdigit());
        if !valid {
            return Err("sha256 must be 32-128 hex characters".into());
        }
    }
    if !MEDIA_STATUSES.contains(&media.status.as_str()) {
        return Err("unknown media status".into());
    }
    if !media.expires_at.is_empty() {
        chrono::DateTime::parse_from_rfc3339(&media.expires_at)
            .map_err(|_| "invalid media expiry timestamp".to_string())?;
    }
    Ok(())
}
