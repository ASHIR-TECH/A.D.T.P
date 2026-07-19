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

#[tauri::command]
pub fn launch_cli(app: AppHandle) -> Result<String, String> {
    // Reuse the tray's launch logic
    crate::tray::launch_cli_terminal(&app);
    Ok("CLI launched".into())
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
}
