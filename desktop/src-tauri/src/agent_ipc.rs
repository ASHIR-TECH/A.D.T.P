//! Local IPC so the co-located `adtp-api` gateway can reach the embedded agent.
//!
//! The LLM agent (`AgentManager` / `AgentRuntime`) lives inside this Tauri
//! process, not in the `adtp-client` daemon, so dispatching `agent_message`
//! over `/tmp/adtp-client.sock` can never reach it. This listener exposes the
//! same line-oriented JSON envelope the daemon uses
//! (`{"cmd": ..., "params": {...}}\n` → `{"ok": ..., "data": ...}\n`) on its
//! own socket, and `adtp-api` prefers it over the daemon socket.

use crate::agent_manager::AgentManager;
use adtp_peer_core::agent::runtime::{ConversationEntry, ToolCallDetail};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

/// Where the gateway looks for this socket (override: `ADTP_AGENT_SOCKET`).
pub const DEFAULT_AGENT_SOCKET: &str = "/tmp/adtp-agent.sock";

pub fn agent_socket_path() -> PathBuf {
    std::env::var("ADTP_AGENT_SOCKET")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_AGENT_SOCKET))
}

#[derive(Debug, Deserialize)]
struct IpcRequest {
    cmd: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct IpcResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[allow(dead_code)]
fn ok(data: Value) -> IpcResponse {
    IpcResponse {
        ok: true,
        data: Some(data),
        error: None,
    }
}

#[allow(dead_code)]
fn err(message: impl Into<String>) -> IpcResponse {
    IpcResponse {
        ok: false,
        data: None,
        error: Some(message.into()),
    }
}

/// Bind the agent socket and serve requests until the process exits.
pub async fn start_agent_ipc_listener(mgr: Arc<Mutex<AgentManager>>) -> Result<(), String> {
    start_agent_ipc_listener_at(mgr, agent_socket_path()).await
}

async fn start_agent_ipc_listener_at(
    mgr: Arc<Mutex<AgentManager>>,
    path: PathBuf,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        use tokio::net::UnixListener;

        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let listener =
            UnixListener::bind(&path).map_err(|e| format!("bind {}: {e}", path.display()))?;
        tracing::info!("ADTP agent IPC listening on {}", path.display());

        loop {
            let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let mgr = mgr.clone();
            tokio::spawn(async move {
                if let Err(error) = handle_stream(stream, mgr).await {
                    tracing::warn!("agent IPC error: {error}");
                }
            });
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (mgr, path);
        Err("agent IPC listener is only implemented for Unix sockets".to_string())
    }
}

#[cfg(unix)]
async fn handle_stream(
    stream: tokio::net::UnixStream,
    mgr: Arc<Mutex<AgentManager>>,
) -> Result<(), String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .map_err(|e| e.to_string())?;

    let response = match serde_json::from_str::<IpcRequest>(&line) {
        Ok(request) => dispatch(&request.cmd, &request.params, &mgr).await,
        Err(error) => err(format!("Invalid JSON request: {error}")),
    };

    let mut encoded = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
    encoded.push(b'\n');
    writer
        .write_all(&encoded)
        .await
        .map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(dead_code)]
async fn dispatch(cmd: &str, params: &Value, mgr: &Arc<Mutex<AgentManager>>) -> IpcResponse {
    match cmd {
        "agent_status" => {
            let guard = mgr.lock().await;
            let (active, provider, model) = guard.status().await;
            ok(json!({ "active": active, "provider": provider, "model": model }))
        }
        "agent_message" => {
            let Some(message) = params.get("message").and_then(Value::as_str) else {
                return err("Missing message");
            };

            let guard = mgr.lock().await;
            let before = guard.get_conversation(None).await.len();
            match guard.send_message(None, message.to_string()).await {
                Ok(text) => {
                    let (_, provider, model) = guard.status().await;
                    let conversation = guard.get_conversation(None).await;
                    ok(json!({
                        "text": text,
                        "message": text.clone(),
                        "provider": provider,
                        "model": model,
                        "tool_calls": collect_tool_calls(&conversation, before),
                    }))
                }
                Err(error) => err(error),
            }
        }
        other => err(format!("Unknown command: {other}")),
    }
}

/// Flattens the tool invocations recorded since `from` into the shape the
/// gateway persists and MARS renders (id/name/status/params/result).
#[allow(dead_code)]
fn collect_tool_calls(conversation: &[ConversationEntry], from: usize) -> Value {
    let calls: Vec<Value> = conversation
        .iter()
        .skip(from)
        .flat_map(|entry| entry.tool_details.iter())
        .map(tool_call_json)
        .collect();
    Value::Array(calls)
}

#[allow(dead_code)]
fn tool_call_json(detail: &ToolCallDetail) -> Value {
    json!({
        "id": detail.id,
        "name": detail.name,
        "status": detail.status,
        "params": detail.params,
        "result": detail.result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use adtp_peer_core::agent::runtime::ToolCallDetail;

    fn entry(tool_details: Vec<ToolCallDetail>) -> ConversationEntry {
        ConversationEntry {
            role: "assistant".into(),
            content: "ok".into(),
            timestamp: 0,
            tool_calls: tool_details.iter().map(|d| d.name.clone()).collect(),
            tool_details,
        }
    }

    #[test]
    fn test_collect_tool_calls_only_includes_new_turn() {
        let old = entry(vec![ToolCallDetail {
            id: "old".into(),
            name: "list_peers".into(),
            status: "completed".into(),
            params: json!({}),
            result: Some("[]".into()),
        }]);
        let new = entry(vec![ToolCallDetail {
            id: "new".into(),
            name: "send_file".into(),
            status: "completed".into(),
            params: json!({"file_path": "/tmp/report.pdf", "peer_id": "abc"}),
            result: Some("queued".into()),
        }]);
        let conversation = vec![old, new];

        let value = collect_tool_calls(&conversation, 1);
        let calls = value.as_array().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["name"], "send_file");
        assert_eq!(calls[0]["params"]["file_path"], "/tmp/report.pdf");
        assert_eq!(calls[0]["status"], "completed");
        assert_eq!(calls[0]["result"], "queued");
    }

    #[tokio::test]
    async fn test_agent_ipc_round_trip_unknown_command() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("agent.sock");
        let mgr = Arc::new(Mutex::new(AgentManager::new()));

        let path = socket.clone();
        let listener = tokio::spawn(async move {
            let _ = start_agent_ipc_listener_at(mgr, path).await;
        });

        for _ in 0..100 {
            if socket.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let stream = tokio::net::UnixStream::connect(&socket).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        writer
            .write_all(b"{\"cmd\":\"bogus\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut line = String::new();
        BufReader::new(reader).read_line(&mut line).await.unwrap();

        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["ok"], false);
        assert!(response["error"]
            .as_str()
            .unwrap()
            .contains("Unknown command: bogus"));

        listener.abort();
    }

    #[tokio::test]
    async fn test_agent_status_envelope_shape() {
        let mgr = Arc::new(Mutex::new(AgentManager::new()));
        let response = dispatch("agent_status", &json!({}), &mgr).await;
        assert!(response.ok);
        let data = response.data.unwrap();
        assert!(data["active"].is_boolean());
    }
}
