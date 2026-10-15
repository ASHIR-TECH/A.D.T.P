use adtp_cli::client::ApiClient;
use adtp_cli::config::CliConfig;
use adtp_cli::models::{
    AuditLogList, ChainVerifyResult, ContextBlock, SessionList, TransferList, TransferSendResponse,
    TransferStatus, WatchList, WatchResponse,
};
use adtp_peer_core::agent::permissions::PermissionConfig;
use tauri::AppHandle;
use tauri::State;

use crate::agent_manager::{AddAgentRequest, AgentManager};
use crate::poller::{HealthSnapshot, SharedState};
use crate::settings;

type AgentState<'a> = State<'a, std::sync::Arc<tokio::sync::Mutex<AgentManager>>>;

fn make_client(app: &AppHandle) -> Result<ApiClient, String> {
    let s = settings::read_settings(app);
    let token = settings::load_api_token(app).ok_or("API token not set")?;
    let cfg = CliConfig {
        api_url: s.api_url,
        api_token: token,
        ..CliConfig::default()
    };
    ApiClient::new(&cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_file(
    app: AppHandle,
    file_path: String,
    agent: Option<String>,
    priority: Option<String>,
    message: Option<String>,
) -> Result<TransferSendResponse, String> {
    let client = make_client(&app)?;
    let body = serde_json::json!({
        "file_path": file_path,
        "agent_name": agent.unwrap_or_else(|| "default".into()),
        "priority": priority.unwrap_or_else(|| "normal".into()),
        "message": message,
    });
    client
        .post("/api/v1/transfers/send", &body)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_transfers(
    app: AppHandle,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<TransferList, String> {
    let client = make_client(&app)?;
    let limit_s = limit.unwrap_or(100).to_string();
    let offset_s = offset.unwrap_or(0).to_string();
    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(ref s) = status {
        params.push(("status", s));
    }
    params.push(("limit", &limit_s));
    params.push(("offset", &offset_s));
    client
        .get_with_query("/api/v1/transfers", &params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_transfer_status(app: AppHandle, id: String) -> Result<TransferStatus, String> {
    let client = make_client(&app)?;
    client
        .get(&format!("/api/v1/transfers/{id}/status"))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_transfer_context(app: AppHandle, id: String) -> Result<ContextBlock, String> {
    let client = make_client(&app)?;
    client
        .get(&format!("/api/v1/transfers/{id}/context"))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_transfer(app: AppHandle, id: String) -> Result<bool, String> {
    let client = make_client(&app)?;
    client
        .delete(&format!("/api/v1/transfers/{id}"))
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn register_watch(
    app: AppHandle,
    folder_path: String,
    file_pattern: Option<String>,
    agent_name: Option<String>,
) -> Result<WatchResponse, String> {
    let client = make_client(&app)?;
    let body = serde_json::json!({
        "folder_path": folder_path,
        "file_pattern": file_pattern,
        "agent_name": agent_name.unwrap_or_else(|| "default".into()),
    });
    client
        .post("/api/v1/watchers", &body)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_watch(app: AppHandle, id: String) -> Result<bool, String> {
    let client = make_client(&app)?;
    client
        .delete(&format!("/api/v1/watchers/{id}"))
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn list_watchers(app: AppHandle) -> Result<WatchList, String> {
    let client = make_client(&app)?;
    client
        .get("/api/v1/watchers")
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sessions(app: AppHandle) -> Result<SessionList, String> {
    let client = make_client(&app)?;
    client
        .get("/api/v1/sessions")
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_audit_log(
    app: AppHandle,
    status: Option<String>,
    session_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<AuditLogList, String> {
    let client = make_client(&app)?;
    let limit_s = limit.unwrap_or(100).to_string();
    let offset_s = offset.unwrap_or(0).to_string();
    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(ref s) = status {
        params.push(("status", s));
    }
    if let Some(ref s) = session_id {
        params.push(("session_id", s));
    }
    params.push(("limit", &limit_s));
    params.push(("offset", &offset_s));
    client
        .get_with_query("/api/v1/audit/log", &params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn verify_chain(app: AppHandle) -> Result<ChainVerifyResult, String> {
    let client = make_client(&app)?;
    client
        .post("/api/v1/audit/verify", &serde_json::json!({}))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_audit(
    app: AppHandle,
    status: Option<String>,
    session_id: Option<String>,
) -> Result<String, String> {
    let client = make_client(&app)?;
    let mut path = "/api/v1/audit/export".to_string();
    let mut params: Vec<String> = Vec::new();
    if let Some(ref s) = status {
        params.push(format!("status={}", s));
    }
    if let Some(ref s) = session_id {
        params.push(format!("session_id={}", s));
    }
    if !params.is_empty() {
        path.push_str(&format!("?{}", params.join("&")));
    }
    client.get_raw(&path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_file(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let file = app
        .dialog()
        .file()
        .add_filter("All Files", &["*"])
        .blocking_pick_file();
    Ok(file.and_then(|p| p.as_path().map(|p| p.to_string_lossy().to_string())))
}

#[tauri::command]
pub async fn pick_save_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let folder = app.dialog().file().blocking_pick_folder();
    Ok(folder.and_then(|p| p.as_path().map(|p| p.to_string_lossy().to_string())))
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    new_settings: settings::AppSettings,
) -> Result<bool, String> {
    settings::write_settings(&app, &new_settings)?;
    Ok(true)
}

#[tauri::command]
pub async fn load_settings(app: AppHandle) -> Result<settings::AppSettings, String> {
    Ok(settings::read_settings(&app))
}

#[tauri::command]
pub async fn save_api_token(app: AppHandle, token: String) -> Result<bool, String> {
    settings::save_api_token(&app, &token)?;
    settings::write_token_to_cli_config(&token)?;
    Ok(true)
}

#[tauri::command]
pub async fn load_api_token(app: AppHandle) -> Result<Option<String>, String> {
    Ok(settings::load_api_token(&app))
}

#[tauri::command]
pub fn generate_api_token() -> String {
    settings::generate_token()
}

#[tauri::command]
pub async fn write_file(path: String, content: String) -> Result<bool, String> {
    std::fs::write(&path, &content).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn get_app_health(
    state: tauri::State<'_, SharedState>,
) -> Result<Option<HealthSnapshot>, String> {
    let s = state.lock().await;
    Ok(s.last_health.clone())
}

#[tauri::command]
pub async fn agent_list(
    agent: AgentState<'_>,
) -> Result<Vec<adtp_peer_core::agent::multi_config::AgentEntry>, String> {
    let mgr = agent.lock().await;
    Ok(mgr.list_agents().await)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAddRequest {
    provider: String,
    model: String,
    api_key: String,
    ollama_host: Option<String>,
    max_tokens: u32,
    permissions: PermissionConfig,
    enabled: bool,
}

#[tauri::command]
pub async fn agent_add(
    agent: AgentState<'_>,
    request: AgentAddRequest,
) -> Result<adtp_peer_core::agent::multi_config::AgentEntry, String> {
    let mgr = agent.lock().await;
    mgr.add_agent(AddAgentRequest {
        provider: request.provider,
        model: request.model,
        api_key: request.api_key,
        ollama_host: request.ollama_host,
        max_tokens: request.max_tokens,
        permissions: request.permissions,
        enabled: request.enabled,
    })
    .await
}

#[tauri::command]
pub async fn agent_remove(agent: AgentState<'_>, id: String) -> Result<bool, String> {
    let mgr = agent.lock().await;
    mgr.remove_agent(id).await?;
    Ok(true)
}

#[tauri::command]
pub async fn agent_toggle(
    agent: AgentState<'_>,
    id: String,
    enabled: bool,
) -> Result<bool, String> {
    let mgr = agent.lock().await;
    mgr.toggle_agent(id, enabled).await?;
    Ok(true)
}

#[tauri::command]
pub async fn agent_send_message(
    agent: AgentState<'_>,
    message: String,
    agent_id: Option<String>,
) -> Result<String, String> {
    let mgr = agent.lock().await;
    mgr.send_message(agent_id, message).await
}

#[tauri::command]
pub async fn agent_get_permissions(_agent: AgentState<'_>) -> Result<PermissionConfig, String> {
    Ok(adtp_peer_core::agent::permissions::load_permissions().unwrap_or_default())
}

#[tauri::command]
pub async fn agent_get_conversation(
    agent: AgentState<'_>,
    agent_id: Option<String>,
) -> Result<Vec<adtp_peer_core::agent::runtime::ConversationEntry>, String> {
    let mgr = agent.lock().await;
    Ok(mgr.get_conversation(agent_id).await)
}

#[tauri::command]
pub async fn agent_clear_conversation(
    agent: AgentState<'_>,
    agent_id: Option<String>,
) -> Result<bool, String> {
    let mgr = agent.lock().await;
    mgr.clear_conversation(agent_id).await;
    Ok(true)
}

#[tauri::command]
pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or("Cannot determine home directory".into())
}

type P2PState<'a> = State<'a, Option<std::sync::Arc<crate::p2p::P2PModule>>>;

#[tauri::command]
pub async fn get_own_peer_id(p2p: P2PState<'_>) -> Result<String, String> {
    match p2p.as_ref() {
        Some(m) => Ok(m.peer_id()),
        None => Err("P2P module not initialized".into()),
    }
}

#[tauri::command]
pub async fn list_peers(p2p: P2PState<'_>) -> Result<Vec<adtp_core::types::PeerEntry>, String> {
    match p2p.as_ref() {
        Some(_module) => {
            let config_dir = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("adtp");
            let peers_path = config_dir.join("peers.toml");
            let trust_store = adtp_peer_core::trust::TrustStore::load(&peers_path);
            let peers: Vec<adtp_core::types::PeerEntry> =
                trust_store.list_peers().into_iter().cloned().collect();
            Ok(peers)
        }
        None => Ok(vec![]),
    }
}

#[derive(serde::Serialize)]
pub struct ConnectionStatus {
    pub rendezvous_registered: bool,
    pub connected_peers: u32,
    pub endpoint_port: u16,
}

#[tauri::command]
pub async fn get_connection_status(p2p: P2PState<'_>) -> Result<ConnectionStatus, String> {
    match p2p.as_ref() {
        Some(module) => {
            let peers = module.list_connected_peers().await;
            Ok(ConnectionStatus {
                rendezvous_registered: true,
                connected_peers: peers.len() as u32,
                endpoint_port: adtp_core::types::ADTP_DEFAULT_PORT,
            })
        }
        None => Err("P2P module not initialized".into()),
    }
}

#[tauri::command]
pub async fn send_file_to_peer(
    p2p: P2PState<'_>,
    file_path: String,
    peer_id: String,
) -> Result<String, String> {
    match p2p.as_ref() {
        Some(module) => module.send_file_to_peer(&file_path, &peer_id).await,
        None => Err("P2P module not initialized".into()),
    }
}

#[tauri::command]
pub async fn add_peer(
    peer_id: String,
    public_key: String,
    host: String,
    port: u16,
    nickname: Option<String>,
    trust_level: String,
) -> Result<bool, String> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("adtp");
    let peers_path = config_dir.join("peers.toml");
    let mut trust_store = adtp_peer_core::trust::TrustStore::load(&peers_path);

    let level = match trust_level.to_lowercase().as_str() {
        "trusted" => adtp_core::types::TrustLevel::Trusted,
        "blocked" => adtp_core::types::TrustLevel::Blocked,
        _ => adtp_core::types::TrustLevel::Unverified,
    };

    trust_store.add_peer(adtp_core::types::PeerEntry {
        peer_id,
        public_key,
        nickname,
        host,
        port,
        trust_level: level,
    });
    trust_store
        .save()
        .map_err(|e| format!("Failed to save peers: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn remove_peer(peer_id: String) -> Result<bool, String> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("adtp");
    let peers_path = config_dir.join("peers.toml");
    let mut trust_store = adtp_peer_core::trust::TrustStore::load(&peers_path);
    trust_store.remove_peer(&peer_id);
    trust_store
        .save()
        .map_err(|e| format!("Failed to save peers: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn get_own_nat_type(p2p: P2PState<'_>) -> Result<String, String> {
    match p2p.as_ref() {
        Some(m) => {
            let nt = m.nat_type().await;
            Ok(format!("{:?}", nt))
        }
        None => Err("P2P module not initialized".into()),
    }
}

#[derive(serde::Serialize)]
pub struct AgentStatusInfo {
    pub agent_count: usize,
    pub active_count: usize,
    pub states: Vec<String>,
}

#[tauri::command]
pub async fn get_agent_status(agent: AgentState<'_>) -> Result<AgentStatusInfo, String> {
    let mgr = agent.lock().await;
    let agents = mgr.list_agents().await;
    let active_count = agents.iter().filter(|a| a.enabled).count();
    Ok(AgentStatusInfo {
        agent_count: agents.len(),
        active_count,
        states: agents
            .iter()
            .map(|a| {
                if a.enabled {
                    "active".into()
                } else {
                    "inactive".into()
                }
            })
            .collect(),
    })
}

/// Whether the built-in shared assistant ("Base AI") is installed and whether
/// it has been permanently retired by the user adding their own provider keys.
#[derive(serde::Serialize)]
pub struct BaseAiStatusInfo {
    pub installed: bool,
    pub retired: bool,
    pub active: bool,
    pub name: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[tauri::command]
pub async fn get_base_ai_status(agent: AgentState<'_>) -> Result<BaseAiStatusInfo, String> {
    let mgr = agent.lock().await;
    let agents = mgr.list_agents().await;
    let entry = agents
        .iter()
        .find(|a| a.provider == adtp_peer_core::agent::base::BASE_PROVIDER);

    Ok(BaseAiStatusInfo {
        installed: entry.is_some(),
        retired: mgr.base_ai_retired().await,
        active: entry.map(|a| a.enabled).unwrap_or(false),
        name: entry.map(|a| a.name.clone()),
        provider: entry.map(|a| a.provider.clone()),
        model: entry.map(|a| a.model.clone()),
    })
}

#[tauri::command]
pub fn launch_cli(app: AppHandle) -> Result<String, String> {
    // Reuse the tray's launch logic
    crate::tray::launch_cli_terminal(&app);
    Ok("CLI launched".into())
}

const PAIRING_QR_VALIDITY_SECS: i64 = 120;

const DEFAULT_API_PORT: u16 = 40003;
const DEFAULT_DESKTOP_NAME: &str = "adtp-pc";

fn primary_non_loopback_ipv4() -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:53").ok()?;
    let local = sock.local_addr().ok()?;
    match local.ip() {
        std::net::IpAddr::V4(v4) if !v4.is_loopback() => Some(v4.to_string()),
        _ => None,
    }
}

fn agent_api_url_from_env() -> String {
    if let Ok(override_url) = std::env::var("ADTP_API_URL") {
        let trimmed = override_url.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let scheme = match std::env::var("ADTP_API_SCHEME").ok().as_deref() {
        Some(s) if s.eq_ignore_ascii_case("https") => "https",
        _ => "http",
    };
    let port = std::env::var("ADTP_API_PORT")
        .ok()
        .and_then(|p| p.trim().parse::<u16>().ok())
        .unwrap_or(DEFAULT_API_PORT);
    let ip = primary_non_loopback_ipv4().unwrap_or_else(|| "127.0.0.1".to_string());
    format!("{}://{}:{}", scheme, ip, port)
}

fn agent_api_token_from_env() -> Result<String, String> {
    match std::env::var("ADTP_API_TOKEN") {
        Ok(token) if token.trim().len() >= 32 => Ok(token.trim().to_string()),
        Ok(_) => Err(
            "ADTP_API_TOKEN is shorter than 32 characters. Set the same token adtp-api runs with."
                .to_string(),
        ),
        Err(_) => Err(
            "ADTP_API_TOKEN is not set. Set the same token adtp-api runs with.".to_string(),
        ),
    }
}

/// QR bearer token: env `ADTP_API_TOKEN` wins, then the token saved in the
/// desktop Settings store. The stored token is the same value this app's own
/// gateway poller authenticates with — so when the desktop UI is working
/// against adtp-api, the QR is guaranteed to carry a token the gateway accepts
/// even if the GUI process was launched without the env var exported.
fn resolve_agent_api_token(stored_token: Option<String>) -> Result<String, String> {
    match agent_api_token_from_env() {
        ok @ Ok(_) => ok,
        Err(env_err) => match stored_token {
            Some(stored) if stored.trim().len() >= 32 => Ok(stored.trim().to_string()),
            _ => Err(env_err),
        },
    }
}

fn build_pairing_payload(
    desktop_id: String,
    desktop_name: String,
    stored_token: Option<String>,
    validity_secs: Option<i64>,
) -> Result<PairingPayload, String> {
    let agent_api_url = agent_api_url_from_env();
    let agent_api_token = resolve_agent_api_token(stored_token)?;

    let now = chrono::Utc::now();
    let validity = validity_secs
        .unwrap_or(PAIRING_QR_VALIDITY_SECS)
        .clamp(10, 3600);

    Ok(PairingPayload {
        version: 1,
        desktop_id,
        desktop_name,
        pairing_token: settings::generate_token(),
        // Millisecond `Z` timestamps are the canonical ES date-time format.
        // MARS runs on Hermes, whose Date.parse is strictly spec-shaped —
        // nanosecond fractions (`+00:00` with 9 digits) are a known risk
        // there, and a failed `expiresAt` parse makes every scan look expired.
        issued_at: now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        expires_at: (now + chrono::Duration::seconds(validity))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        relay_url: "wss://relay.ashir.world:443/adtp".to_string(),
        agent_api_url,
        agent_api_token,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingPayload {
    pub version: u32,
    pub desktop_id: String,
    pub desktop_name: String,
    pub pairing_token: String,
    pub issued_at: String,
    pub expires_at: String,
    pub relay_url: String,
    pub agent_api_url: String,
    pub agent_api_token: String,
}

#[tauri::command]
pub async fn get_pairing_qr_data(
    app: AppHandle,
    p2p: P2PState<'_>,
    validity_secs: Option<i64>,
) -> Result<PairingPayload, String> {
    let module = p2p.as_ref().ok_or("P2P module not initialized")?;

    let desktop_name = match hostname::get() {
        Ok(name) => name.to_string_lossy().to_string(),
        Err(_) => DEFAULT_DESKTOP_NAME.to_string(),
    };

    build_pairing_payload(
        module.peer_id(),
        desktop_name,
        settings::load_api_token(&app),
        validity_secs,
    )
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestResult {
        path: Option<String>,
        cancelled: bool,
    }

    #[test]
    fn test_pick_file_result_serde() {
        let r = TestResult {
            path: Some("/tmp/test.txt".into()),
            cancelled: false,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["path"], "/tmp/test.txt");
        assert_eq!(json["cancelled"], false);
    }

    #[test]
    fn test_pick_file_result_cancelled() {
        let r = TestResult {
            path: None,
            cancelled: true,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert!(json["path"].is_null());
        assert_eq!(json["cancelled"], true);
    }

    fn with_env<K: AsRef<std::ffi::OsStr>>(key: K, value: &str, f: impl FnOnce()) {
        let key = key.as_ref();
        let prev = std::env::var_os(key);
        std::env::set_var(key, value);
        f();
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_agent_api_url_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        with_env("ADTP_API_URL", "http://192.168.1.50:9000", || {
            assert_eq!(super::agent_api_url_from_env(), "http://192.168.1.50:9000");
        });
    }

    #[test]
    fn test_agent_api_url_scheme_and_port() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_url = std::env::var_os("ADTP_API_URL");
        std::env::remove_var("ADTP_API_URL");
        let prev_scheme = std::env::var_os("ADTP_API_SCHEME");
        let prev_port = std::env::var_os("ADTP_API_PORT");
        std::env::set_var("ADTP_API_SCHEME", "https");
        std::env::set_var("ADTP_API_PORT", "40005");
        let url = super::agent_api_url_from_env();
        std::env::remove_var("ADTP_API_SCHEME");
        std::env::remove_var("ADTP_API_PORT");
        if let Some(v) = prev_scheme {
            std::env::set_var("ADTP_API_SCHEME", v);
        }
        if let Some(v) = prev_port {
            std::env::set_var("ADTP_API_PORT", v);
        }
        if let Some(v) = prev_url {
            std::env::set_var("ADTP_API_URL", v);
        }
        assert!(url.starts_with("https://"), "unexpected url: {url}");
        assert!(url.ends_with(":40005"), "unexpected url: {url}");
    }

    #[test]
    fn test_agent_api_token_validation() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("ADTP_API_TOKEN");
        std::env::remove_var("ADTP_API_TOKEN");
        assert!(super::agent_api_token_from_env().is_err());
        std::env::set_var("ADTP_API_TOKEN", "short");
        assert!(super::agent_api_token_from_env().is_err());
        std::env::set_var("ADTP_API_TOKEN", "abcdefghijklmnopqrstuvwxyz0123456789");
        assert_eq!(
            super::agent_api_token_from_env().unwrap(),
            "abcdefghijklmnopqrstuvwxyz0123456789"
        );
        if let Some(v) = prev {
            std::env::set_var("ADTP_API_TOKEN", v);
        }
    }

    #[test]
    fn test_pairing_payload_serializes_agent_fields() {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Mini {
            agent_api_url: String,
            agent_api_token: String,
        }
        let v = serde_json::to_value(Mini {
            agent_api_url: "http://10.0.0.5:40003".into(),
            agent_api_token: "x".repeat(32),
        })
        .unwrap();
        assert_eq!(v["agentApiUrl"], "http://10.0.0.5:40003");
        assert_eq!(v["agentApiToken"], "x".repeat(32));
    }

    #[test]
    fn test_resolve_agent_api_token_env_wins_then_stored() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("ADTP_API_TOKEN");

        let env_token = format!("env-{}", "e".repeat(36));
        let stored_token = format!("stored-{}", "s".repeat(36));

        std::env::set_var("ADTP_API_TOKEN", &env_token);
        assert_eq!(
            super::resolve_agent_api_token(Some(stored_token.clone())).unwrap(),
            env_token
        );

        std::env::remove_var("ADTP_API_TOKEN");
        assert_eq!(
            super::resolve_agent_api_token(Some(stored_token.clone())).unwrap(),
            stored_token
        );

        // A short/blank stored token never falls through silently.
        assert!(super::resolve_agent_api_token(Some("short".into())).is_err());
        assert!(super::resolve_agent_api_token(None).is_err());

        match prev {
            Some(v) => std::env::set_var("ADTP_API_TOKEN", v),
            None => std::env::remove_var("ADTP_API_TOKEN"),
        }
    }

    /// The payload must match the mobile parser exactly (MARS
    /// `src/pairing/parsePairingPayload.ts`): exact camelCase key set,
    /// `version: 1`, and canonical ES timestamps Hermes can parse.
    #[test]
    fn test_build_pairing_payload_matches_mobile_schema() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_token = std::env::var_os("ADTP_API_TOKEN");
        let prev_url = std::env::var_os("ADTP_API_URL");
        let api_url = std::env::var("ADTP_API_URL")
            .unwrap_or_else(|_| "http://10.0.0.7:40003".to_string());
        std::env::set_var("ADTP_API_TOKEN", "z".repeat(40));
        std::env::set_var("ADTP_API_URL", &api_url);

        let payload = super::build_pairing_payload(
            "a".repeat(64),
            "test-pc".into(),
            Some("unused-stored".into()),
            Some(3600),
        )
        .unwrap();
        let json = serde_json::to_value(&payload).unwrap();

        let mut keys: Vec<String> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "agentApiToken",
                "agentApiUrl",
                "desktopId",
                "desktopName",
                "expiresAt",
                "issuedAt",
                "pairingToken",
                "relayUrl",
                "version",
            ]
        );
        assert_eq!(json["version"], 1);
        assert_eq!(json["desktopId"].as_str().unwrap().len(), 64);
        assert_eq!(json["agentApiUrl"], api_url.as_str());
        assert_eq!(json["agentApiToken"], "z".repeat(40));
        assert!(json["relayUrl"].as_str().unwrap().starts_with("wss://"));

        // `YYYY-MM-DDTHH:MM:SS.mmmZ` — exactly 3 fraction digits + Z.
        for field in ["issuedAt", "expiresAt"] {
            let s = json[field].as_str().unwrap();
            assert_eq!(s.len(), 24, "{field} not canonical ES format: {s}");
            assert!(s.ends_with('Z'), "{field} must end in Z: {s}");
            assert_eq!(s.as_bytes()[19], b'.', "{field} must have .mmm: {s}");
            assert!(
                s.chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, 'T' | ':' | '.' | 'Z' | '-')),
                "{field} has unexpected chars: {s}"
            );
        }

        // Expiry must be in the future and honour the requested validity.
        let issued = chrono::DateTime::parse_from_rfc3339(json["issuedAt"].as_str().unwrap())
            .unwrap()
            .timestamp();
        let expires = chrono::DateTime::parse_from_rfc3339(json["expiresAt"].as_str().unwrap())
            .unwrap()
            .timestamp();
        assert_eq!(expires - issued, 3600);

        // E2E harness hook: print the real serialized payload so the MARS
        // parser + a live adtp-api can be exercised against it (node harness).
        if std::env::var("ADTP_E2E_EMIT").is_ok() {
            println!("ADTP_QR_PAYLOAD={}", serde_json::to_string(&payload).unwrap());
        }

        match prev_token {
            Some(v) => std::env::set_var("ADTP_API_TOKEN", v),
            None => std::env::remove_var("ADTP_API_TOKEN"),
        }
        match prev_url {
            Some(v) => std::env::set_var("ADTP_API_URL", v),
            None => std::env::remove_var("ADTP_API_URL"),
        }
    }
}
