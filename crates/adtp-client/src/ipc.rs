use crate::queue::{QueueItem, TransferQueue};
use adtp_core::{
    errors::{AdtpError, AdtpResult},
    types::{Priority, TransferType},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc, time::Instant};
use uuid::Uuid;

#[derive(Clone)]
pub struct ClientIpcState {
    queue: Arc<TransferQueue>,
    started_at: Instant,
}

impl ClientIpcState {
    pub fn new(queue: Arc<TransferQueue>) -> Self {
        Self {
            queue,
            started_at: Instant::now(),
        }
    }
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

pub async fn start_ipc_listener(state: ClientIpcState) -> AdtpResult<()> {
    #[cfg(unix)]
    {
        use tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
            net::{UnixListener, UnixStream},
        };

        let socket_path = std::env::var("ADTP_CLIENT_SOCKET")
            .unwrap_or_else(|_| "/tmp/adtp-client.sock".to_string());
        let path = PathBuf::from(&socket_path);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let listener = UnixListener::bind(&path)?;
        tracing::info!("ADTP client IPC listening on {}", path.display());

        loop {
            let (stream, _) = listener.accept().await?;
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(error) = handle_stream(stream, state).await {
                    tracing::warn!("Client IPC error: {}", error);
                }
            });
        }

        async fn handle_stream(stream: UnixStream, state: ClientIpcState) -> AdtpResult<()> {
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            let response = match serde_json::from_str::<IpcRequest>(&line) {
                Ok(request) => handle_request(request, &state),
                Err(error) => IpcResponse {
                    ok: false,
                    data: None,
                    error: Some(format!("Invalid JSON request: {error}")),
                },
            };
            let encoded = serde_json::to_vec(&response).map_err(AdtpError::Json)?;
            writer.write_all(&encoded).await?;
            writer.write_all(b"\n").await?;
            Ok(())
        }
    }

    #[cfg(not(unix))]
    {
        let _ = state;
        Err(AdtpError::Internal {
            reason: "Client IPC listener is only implemented for Unix sockets in Phase 2"
                .to_string(),
        })
    }
}

fn handle_request(request: IpcRequest, state: &ClientIpcState) -> IpcResponse {
    match request.cmd.as_str() {
        "send" => handle_send(request.params, state),
        "watch" => ok(json!({
            "watch_id": Uuid::new_v4().to_string(),
            "active": true
        })),
        "unwatch" => ok(json!({
            "watch_id": request.params.get("watch_id").and_then(Value::as_str),
            "removed": true
        })),
        "get_transfers" => ok(json!({ "transfers": [] })),
        "get_context" => ok(json!({
            "file_meta": null,
            "context": {
                "schema": null,
                "summary": null,
                "agent_hint": null,
                "transfer_history": [],
                "custom_metadata": {}
            }
        })),
        "get_status" => ok(json!({
            "status": "unknown",
            "bytes_sent": 0,
            "total_bytes": 0,
            "elapsed_ms": 0
        })),
        "session_status" => ok(json!({
            "peer_addr": null,
            "bytes_sent": 0,
            "bytes_received": 0,
            "transfers_complete": 0,
            "uptime_ms": state.started_at.elapsed().as_millis() as u64
        })),
        "pending_files" => ok(json!({ "pending": [] })),
        other => err(format!("Unknown command: {other}")),
    }
}

fn handle_send(params: Value, state: &ClientIpcState) -> IpcResponse {
    let file_path = match params.get("file_path").and_then(Value::as_str) {
        Some(path) => PathBuf::from(path),
        None => return err("Missing file_path".to_string()),
    };
    let agent_name = params
        .get("agent_name")
        .and_then(Value::as_str)
        .unwrap_or("default")
        .to_string();
    let priority = match params.get("priority").and_then(Value::as_str) {
        Some("urgent") => Priority::Urgent,
        _ => Priority::Normal,
    };
    let mut item = QueueItem::new_send(file_path, agent_name);
    item.priority = priority;
    item.transfer_type = TransferType::Send;
    let transfer_id = item.id;

    match state.queue.enqueue(item) {
        Ok(()) => ok(json!({
            "transfer_id": transfer_id.to_string(),
            "status": "queued",
            "estimated_ms": 0
        })),
        Err(error) => err(error.to_string()),
    }
}

fn ok(data: Value) -> IpcResponse {
    IpcResponse {
        ok: true,
        data: Some(data),
        error: None,
    }
}

fn err(error: String) -> IpcResponse {
    IpcResponse {
        ok: false,
        data: None,
        error: Some(error),
    }
}
