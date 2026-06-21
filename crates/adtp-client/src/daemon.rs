use crate::{
    audit::AuditLog,
    config::ClientConfig,
    ipc::{start_ipc_listener, ClientIpcState},
    queue::{QueueItem, TransferQueue},
    receiver::FileReceiver,
};
use adtp_core::{
    encryption::encrypt_file_bytes,
    errors::{AdtpError, AdtpResult},
    integrity::hash_file,
    protocol::{AdtpPacket, ContextEnvelope, PacketHeader},
    session::{new_session, SessionKeys},
    transfer::{AdtpClient, AdtpConnection},
    types::*,
};
use bytes::Bytes;
use chrono::Utc;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::fs;
use uuid::Uuid;

pub struct ClientDaemon {
    config: ClientConfig,
    queue: Arc<TransferQueue>,
    audit: Arc<AuditLog>,
}

impl ClientDaemon {
    pub fn new(config: ClientConfig) -> AdtpResult<Self> {
        let audit = AuditLog::open(&config.audit_db)?;
        Ok(Self {
            config,
            queue: Arc::new(TransferQueue::new()),
            audit: Arc::new(audit),
        })
    }

    pub async fn run(&self) -> AdtpResult<()> {
        tracing::info!("ADTP Client Daemon starting");

        fs::create_dir_all(&self.config.receive_dir).await?;
        let ipc_state = ClientIpcState::new(self.queue.clone());
        tokio::spawn(async move {
            if let Err(error) = start_ipc_listener(ipc_state).await {
                tracing::error!("Client IPC listener stopped: {}", error);
            }
        });

        let agent = self
            .config
            .agents
            .iter()
            .find(|agent| agent.default)
            .ok_or_else(|| AdtpError::Internal {
                reason: "No default agent configured. Run: adtp connect".to_string(),
            })?;
        let addr: SocketAddr = format!("{}:{}", agent.host, agent.port)
            .parse()
            .map_err(|e| AdtpError::Internal {
                reason: format!("Invalid agent address: {e}"),
            })?;
        let client = AdtpClient::new().await?;
        let connection = Arc::new(client.connect(addr, &agent.host).await?);

        tracing::info!("Connected to agent: {}:{}", agent.host, agent.port);

        let _session_keys = SessionKeys::generate();
        let session = new_session(connection.peer_addr());
        self.audit
            .log_session_start(&session.id, &connection.peer_addr())?;

        let session_key = [0u8; AES_KEY_SIZE];
        let session_id = session.id;
        let conn_send = connection.clone();
        let queue_rx = self.queue.receiver();
        let audit_send = self.audit.clone();

        tokio::spawn(async move {
            send_worker(conn_send, queue_rx, audit_send, session_id, session_key).await;
        });

        let conn_recv = connection.clone();
        let audit_recv = self.audit.clone();
        let receiver = FileReceiver::new(self.config.receive_dir.clone());

        tokio::spawn(async move {
            recv_worker(conn_recv, audit_recv, receiver, session_id, session_key).await;
        });

        tokio::signal::ctrl_c().await?;
        self.audit.log_session_end(&session.id, 0)?;
        tracing::info!("ADTP Client Daemon shutting down");
        Ok(())
    }

    pub fn queue_sender(&self) -> flume::Sender<QueueItem> {
        self.queue.sender()
    }
}

async fn send_worker(
    connection: Arc<AdtpConnection>,
    queue_rx: flume::Receiver<QueueItem>,
    audit: Arc<AuditLog>,
    session_id: Uuid,
    session_key: [u8; AES_KEY_SIZE],
) {
    while let Ok(item) = queue_rx.recv_async().await {
        match send_file(&connection, &item, &audit, session_id, &session_key).await {
            Ok(()) => tracing::info!("Sent: {}", item.file_path.display()),
            Err(error) => tracing::error!("Send failed: {}", error),
        }
    }
}

async fn send_file(
    connection: &AdtpConnection,
    item: &QueueItem,
    audit: &AuditLog,
    session_id: Uuid,
    session_key: &[u8; AES_KEY_SIZE],
) -> AdtpResult<()> {
    let transfer_id = Uuid::new_v4();
    let path = &item.file_path;
    let content_hash = hash_file(path).await?;
    let file_bytes = tokio::fs::read(path).await?;
    let file_size = file_bytes.len() as u64;
    let mime_type = infer::get(&file_bytes)
        .map(|kind| kind.mime_type().to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let file_meta = FileMeta {
        filename: filename.clone(),
        mime_type,
        file_size,
        extension,
        last_modified: None,
        content_hash,
    };
    let context_env = ContextEnvelope {
        file_meta: file_meta.clone(),
        context: ContextBlock::default(),
    };
    let context_bytes = serde_json::to_vec(&context_env).map_err(AdtpError::Json)?;
    let (ciphertext, nonce) = encrypt_file_bytes(session_key, &file_bytes)?;
    let header = PacketHeader::new(
        TransferType::Transfer,
        session_id,
        transfer_id,
        [0u8; PEER_ID_SIZE],
        [0u8; PEER_ID_SIZE],
        nonce,
        content_hash,
        context_bytes.len() as u32,
        ciphertext.len() as u64,
    );
    let packet = AdtpPacket {
        header,
        context: Bytes::from(context_bytes),
        payload: Bytes::from(ciphertext),
        signature: [0u8; ED25519_SIG_SIZE],
    };

    connection.send_packet(&packet).await?;

    let record = TransferRecord {
        id: transfer_id,
        session_id,
        sender_peer_id: [0u8; PEER_ID_SIZE],
        receiver_peer_id: [0u8; PEER_ID_SIZE],
        transfer_type: TransferType::Transfer,
        file_meta,
        status: TransferStatus::Delivered,
        initiated_at: Utc::now(),
        completed_at: Some(Utc::now()),
        signature: hex::encode([0u8; ED25519_SIG_SIZE]),
        received_hash: None,
        integrity_verified: true,
    };
    audit.log_transfer(&record)?;

    Ok(())
}

async fn recv_worker(
    connection: Arc<AdtpConnection>,
    audit: Arc<AuditLog>,
    receiver: FileReceiver,
    session_id: Uuid,
    session_key: [u8; AES_KEY_SIZE],
) {
    let mut transfer_count = 0usize;
    loop {
        match connection.recv_packet().await {
            Ok(packet) => match receiver.handle_push(&packet, &session_key).await {
                Ok(path) => {
                    transfer_count += 1;
                    tracing::info!("Received and saved: {}", path.display());
                }
                Err(error) => tracing::error!("Receive error: {}", error),
            },
            Err(error) => {
                tracing::error!("Connection error: {}", error);
                break;
            }
        }
    }
    let _ = audit.log_session_end(&session_id, transfer_count);
}
