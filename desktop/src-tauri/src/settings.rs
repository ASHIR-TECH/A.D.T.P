use adtp_cli::config::AgentEntry;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";

const TOKEN_KEY: &str = "api_token";
const DESKTOP_ID_KEY: &str = "desktop_id";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub api_url: String,
    pub poll_transfers_ms: u64,
    pub poll_health_ms: u64,
    pub minimize_to_tray: bool,
    pub theme: String,
    pub agents: Vec<AgentEntry>,
    #[serde(default)]
    pub desktop_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            api_url: "http://127.0.0.1:40003".into(),
            poll_transfers_ms: 2000,
            poll_health_ms: 5000,
            minimize_to_tray: true,
            theme: "dark".into(),
            agents: Vec::new(),
            desktop_id: String::new(),
        }
    }
}

pub fn read_settings(app: &AppHandle) -> AppSettings {
    if let Ok(store) = app.store(STORE_FILE) {
        if let Some(val) = store.get("settings") {
            if let Ok(s) = serde_json::from_value(val.clone()) {
                return s;
            }
        }
    }
    AppSettings::default()
}

pub fn write_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    let val = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    store.set("settings", val);
    store.save().map_err(|e| e.to_string())
}

pub fn save_api_token(app: &AppHandle, token: &str) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    store.set(TOKEN_KEY, serde_json::Value::String(token.to_string()));
    store.save().map_err(|e| e.to_string())
}

pub fn load_api_token(app: &AppHandle) -> Option<String> {
    let store = app.store(STORE_FILE).ok()?;
    store
        .get(TOKEN_KEY)
        .and_then(|v| v.as_str().map(String::from))
}

/// Returns a stable desktop identifier used in the pairing QR payload.
///
/// Prefers the P2P peer ID when available; otherwise falls back to a
/// persisted settings value (generated on first launch). This keeps the QR
/// working even when the P2P/QUIC module cannot bind (e.g. port conflict in
/// a packaged install).
pub fn desktop_id(app: &AppHandle) -> String {
    let store = match app.store(STORE_FILE) {
        Ok(s) => s,
        Err(_) => return generate_desktop_id(),
    };
    if let Some(id) = store
        .get(DESKTOP_ID_KEY)
        .and_then(|v| v.as_str().map(String::from))
    {
        if !id.is_empty() {
            return id;
        }
    }
    let id = generate_desktop_id();
    store.set(DESKTOP_ID_KEY, serde_json::Value::String(id.clone()));
    store.save();
    id
}

fn generate_desktop_id() -> String {
    // A hostname-derived stable id so the same machine always shows the same
    // desktop id in the QR, without needing the P2P module.
    match ::hostname::get() {
        Ok(name) => format!("adtp-{}", name.to_string_lossy().replace('.', "-")),
        Err(_) => format!("adtp-{}", generate_token()),
    }
}

pub fn write_token_to_cli_config(token: &str) -> Result<(), String> {
    write_cli_config(token, "")
}

/// Keep the CLI pointed at the same gateway the desktop/QR use. Without the
/// URL sync, a non-default `api_url` (custom port, LAN/EC2 host) made
/// `send_file_to_device` queue files against a gateway that isn't there.
pub fn write_cli_config(token: &str, api_url: &str) -> Result<(), String> {
    let mut cfg = adtp_cli::config::CliConfig::load().map_err(|e| e.to_string())?;
    cfg.api_token = token.to_string();
    let url = api_url.trim().trim_end_matches('/');
    if !url.is_empty() {
        cfg.api_url = url.to_string();
    }
    cfg.save().map_err(|e| e.to_string())
}

pub fn generate_token() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = AppSettings::default();
        assert_eq!(s.api_url, "http://127.0.0.1:40003");
        assert_eq!(s.poll_transfers_ms, 2000);
        assert_eq!(s.poll_health_ms, 5000);
        assert!(s.minimize_to_tray);
        assert_eq!(s.theme, "dark");
        assert!(s.agents.is_empty());
    }

    #[test]
    fn test_settings_round_trip() {
        let s = AppSettings {
            api_url: "http://localhost:9999".into(),
            poll_transfers_ms: 1000,
            poll_health_ms: 3000,
            minimize_to_tray: false,
            theme: "light".into(),
            agents: vec![AgentEntry {
                name: "test-agent".into(),
                host: "10.0.0.1".into(),
                port: 40003,
                default: true,
            }],
            desktop_id: "adtp-test-pc".into(),
        };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["api_url"], "http://localhost:9999");
        let deserialized: AppSettings = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.api_url, s.api_url);
        assert_eq!(deserialized.poll_transfers_ms, s.poll_transfers_ms);
        assert_eq!(deserialized.agents.len(), 1);
        assert_eq!(deserialized.agents[0].name, "test-agent");
    }
}
