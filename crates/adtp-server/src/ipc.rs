use adtp_core::errors::{AdtpError, AdtpResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::PathBuf, time::Instant};
use uuid::Uuid;

#[derive(Clone)]
pub struct ServerIpcState {
    started_at: Instant,
}

impl ServerIpcState {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }
}

impl Default for ServerIpcState {
    fn default() -> Self {
        Self::new()
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

pub async fn start_ipc_listener(state: ServerIpcState) -> AdtpResult<()> {
    #[cfg(unix)]
    {
        use tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
            net::{UnixListener, UnixStream},
        };

        let socket_path = std::env::var("ADTP_SERVER_SOCKET")
            .unwrap_or_else(|_| "/tmp/adtp-server.sock".to_string());
        let path = PathBuf::from(&socket_path);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let listener = UnixListener::bind(&path)?;
        tracing::info!("ADTP server IPC listening on {}", path.display());

        loop {
            let (stream, _) = listener.accept().await?;
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(error) = handle_stream(stream, state).await {
                    tracing::warn!("Server IPC error: {}", error);
                }
            });
        }

        async fn handle_stream(stream: UnixStream, state: ServerIpcState) -> AdtpResult<()> {
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
            reason: "Server IPC listener is only implemented for Unix sockets in Phase 2"
                .to_string(),
        })
    }
}

fn handle_request(request: IpcRequest, state: &ServerIpcState) -> IpcResponse {
    match request.cmd.as_str() {
        "push" => {
            let filename = request
                .params
                .get("file_path")
                .and_then(Value::as_str)
                .and_then(|path| {
                    PathBuf::from(path)
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());
            ok(json!({
                "transfer_id": Uuid::new_v4().to_string(),
                "status": "queued",
                "filename": filename,
                "delivered_to": request.params.get("destination").and_then(Value::as_str)
            }))
        }
        "session_status" => ok(json!({
            "peer_addr": null,
            "bytes_sent": 0,
            "bytes_received": 0,
            "transfers_complete": 0,
            "uptime_ms": state.started_at.elapsed().as_millis() as u64
        })),
        other => err(format!("Unknown command: {other}")),
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
