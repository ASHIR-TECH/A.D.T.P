use adtp_cli::config::AgentEntry;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";

const TOKEN_KEY: &str = "api_token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub api_url: String,
    pub poll_transfers_ms: u64,
    pub poll_health_ms: u64,
    pub minimize_to_tray: bool,
    pub theme: String,
    pub agents: Vec<AgentEntry>,
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
    store.get(TOKEN_KEY).and_then(|v| v.as_str().map(String::from))
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
