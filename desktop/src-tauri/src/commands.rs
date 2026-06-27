use adtp_cli::client::ApiClient;
use adtp_cli::config::CliConfig;
use adtp_cli::models::{
    AuditLogList, ChainVerifyResult, ContextBlock, SessionList, TransferList,
    TransferSendResponse, TransferStatus, WatchList, WatchResponse,
};
use adtp_peer_core::agent::permissions::PermissionConfig;
use tauri::AppHandle;
use tauri::State;

use crate::agent_manager::AgentManager;
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
    client.post("/api/v1/transfers/send", &body).await.map_err(|e| e.to_string())
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
    client.get_with_query("/api/v1/transfers", &params).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_transfer_status(app: AppHandle, id: String) -> Result<TransferStatus, String> {
    let client = make_client(&app)?;
    client.get(&format!("/api/v1/transfers/{id}/status")).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_transfer_context(app: AppHandle, id: String) -> Result<ContextBlock, String> {
    let client = make_client(&app)?;
    client.get(&format!("/api/v1/transfers/{id}/context")).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_transfer(app: AppHandle, id: String) -> Result<bool, String> {
    let client = make_client(&app)?;
    client.delete(&format!("/api/v1/transfers/{id}")).await.map_err(|e| e.to_string())?;
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
    client.post("/api/v1/watchers", &body).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_watch(app: AppHandle, id: String) -> Result<bool, String> {
    let client = make_client(&app)?;
    client.delete(&format!("/api/v1/watchers/{id}")).await.map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn list_watchers(app: AppHandle) -> Result<WatchList, String> {
    let client = make_client(&app)?;
    client.get("/api/v1/watchers").await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sessions(app: AppHandle) -> Result<SessionList, String> {
    let client = make_client(&app)?;
    client.get("/api/v1/sessions").await.map_err(|e| e.to_string())
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
    if let Some(ref s) = status { params.push(("status", s)); }
    if let Some(ref s) = session_id { params.push(("session_id", s)); }
    params.push(("limit", &limit_s));
    params.push(("offset", &offset_s));
    client.get_with_query("/api/v1/audit/log", &params).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn verify_chain(app: AppHandle) -> Result<ChainVerifyResult, String> {
    let client = make_client(&app)?;
    client.post("/api/v1/audit/verify", &serde_json::json!({})).await.map_err(|e| e.to_string())
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
    if let Some(ref s) = status { params.push(format!("status={}", s)); }
    if let Some(ref s) = session_id { params.push(format!("session_id={}", s)); }
    if !params.is_empty() {
        path.push_str(&format!("?{}", params.join("&")));
    }
    client.get_raw(&path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_file(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let file = app.dialog()
        .file()
        .add_filter("All Files", &["*"])
        .blocking_pick_file();
    Ok(file.and_then(|p| p.as_path().map(|p| p.to_string_lossy().to_string())))
}

#[tauri::command]
pub async fn pick_save_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let folder = app.dialog()
        .file()
        .blocking_pick_folder();
    Ok(folder.and_then(|p| p.as_path().map(|p| p.to_string_lossy().to_string())))
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, new_settings: settings::AppSettings) -> Result<bool, String> {
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
    Ok(true)
}

#[tauri::command]
pub async fn load_api_token(app: AppHandle) -> Result<Option<String>, String> {
    Ok(settings::load_api_token(&app))
}

#[tauri::command]
pub async fn write_file(path: String, content: String) -> Result<bool, String> {
    std::fs::write(&path, &content).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn get_app_health(state: tauri::State<'_, SharedState>) -> Result<Option<HealthSnapshot>, String> {
    let s = state.lock().await;
    Ok(s.last_health.clone())
}

#[tauri::command]
pub async fn agent_get_status(
    agent: AgentState<'_>,
) -> Result<serde_json::Value, String> {
    let mgr = agent.lock().await;
    Ok(mgr.status().await)
}

#[tauri::command]
pub async fn agent_activate(
    agent: AgentState<'_>,
    provider: String,
    model: String,
    api_key: String,
    ollama_host: Option<String>,
    max_tokens: u32,
    permissions: PermissionConfig,
) -> Result<bool, String> {
    let mgr = agent.lock().await;
    mgr.activate(provider, model, api_key, ollama_host, max_tokens, permissions).await?;
    Ok(true)
}

#[tauri::command]
pub async fn agent_deactivate(
    agent: AgentState<'_>,
) -> Result<bool, String> {
    let mgr = agent.lock().await;
    mgr.deactivate().await?;
    Ok(true)
}

#[tauri::command]
pub async fn agent_send_message(
    agent: AgentState<'_>,
    message: String,
) -> Result<String, String> {
    let mgr = agent.lock().await;
    mgr.send_message(message).await
}

#[tauri::command]
pub async fn agent_get_permissions(
    agent: AgentState<'_>,
) -> Result<PermissionConfig, String> {
    let mgr = agent.lock().await;
    Ok(mgr.permissions().await)
}

#[tauri::command]
pub async fn agent_update_permissions(
    agent: AgentState<'_>,
    config: PermissionConfig,
) -> Result<bool, String> {
    let mgr = agent.lock().await;
    mgr.update_permissions(config).await?;
    Ok(true)
}

#[tauri::command]
pub async fn agent_get_conversation(
    agent: AgentState<'_>,
) -> Result<Vec<adtp_peer_core::agent::runtime::ConversationEntry>, String> {
    let mgr = agent.lock().await;
    Ok(mgr.conversation().await)
}

#[tauri::command]
pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir().map(|p| p.to_string_lossy().to_string()).ok_or("Cannot determine home directory".into())
}

#[tauri::command]
pub async fn agent_clear_conversation(
    agent: AgentState<'_>,
) -> Result<bool, String> {
    let mgr = agent.lock().await;
    mgr.clear_conversation().await;
    Ok(true)
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
