use crate::{
    audit::AuditLog,
    config::ServerConfig,
    dispatcher::Dispatcher,
    intake::FileIntake,
    ipc::{start_ipc_listener, ServerIpcState},
};
use adtp_core::{
    errors::{AdtpError, AdtpResult},
    session::{new_session, server_handshake},
    transfer::{AdtpConnection, AdtpServer},
    types::{TransferStatus, TransferType},
};
use chrono::Utc;
use std::net::SocketAddr;
use std::sync::Arc;

pub struct ServerDaemon {
    config: ServerConfig,
}

impl ServerDaemon {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> AdtpResult<()> {
        tracing::info!("ADTP Server Daemon starting");

        let addr: SocketAddr = format!("{}:{}", self.config.bind_addr, self.config.port)
            .parse()
            .map_err(|e| AdtpError::Internal {
                reason: format!("Invalid bind address: {e}"),
            })?;
        let server = AdtpServer::bind(addr).await?;
        let audit = Arc::new(AuditLog::open(&self.config.audit_db)?);
        let intake = Arc::new(FileIntake::new(self.config.intake_dir.clone()));
        let dispatcher = Arc::new(Dispatcher::new(self.config.output_dir.clone()));
        tokio::spawn(async move {
            if let Err(error) = start_ipc_listener(ServerIpcState::new()).await {
                tracing::error!("Server IPC listener stopped: {}", error);
            }
        });

        tracing::info!("Listening on {}", addr);

        while let Ok(Some(connection)) = server.accept().await {
            let connection = Arc::new(connection);
            let audit = audit.clone();
            let intake = intake.clone();
            let dispatcher = dispatcher.clone();
            let config = self.config.clone();

            tokio::spawn(async move {
                handle_client(connection, audit, intake, dispatcher, config).await;
            });
        }

        Ok(())
    }
}

async fn handle_client(
    connection: Arc<AdtpConnection>,
    audit: Arc<AuditLog>,
    intake: Arc<FileIntake>,
    dispatcher: Arc<Dispatcher>,
    config: ServerConfig,
) {
    let session = new_session(connection.peer_addr());
    let _ = audit.log_session_start(&session.id, &connection.peer_addr());

    let session_key = match server_handshake(&connection, &session.id).await {
        Ok(key) => key,
        Err(e) => {
            tracing::error!("Session key exchange failed: {}", e);
            return;
        }
    };
    let mut transfer_count = 0usize;

    tracing::info!("Client connected: {}", connection.peer_addr());

    loop {
        match connection.recv_packet().await {
            Ok(packet) => match packet.header.transfer_type {
                TransferType::Transfer => match intake.process(&packet, &session_key).await {
                    Ok(result) => {
                        transfer_count += 1;
                        if result.file_meta.file_size > config.max_file_size {
                            tracing::warn!(
                                "Received file exceeds configured size limit: {}",
                                result.file_meta.filename
                            );
                        }
                        let record = adtp_core::types::TransferRecord {
                            id: packet.header.transfer_id,
                            session_id: session.id,
                            sender_peer_id: packet.header.sender_peer_id,
                            receiver_peer_id: packet.header.receiver_peer_id,
                            transfer_type: TransferType::Transfer,
                            file_meta: result.file_meta.clone(),
                            status: TransferStatus::Delivered,
                            initiated_at: Utc::now(),
                            completed_at: Some(Utc::now()),
                            signature: hex::encode(packet.signature),
                            received_hash: Some(packet.header.content_hash),
                            integrity_verified: true,
                        };
                        if let Err(error) = audit.log_transfer(&record) {
                            tracing::error!("Audit error: {}", error);
                        }
                        if let Err(error) = dispatcher.dispatch(&result).await {
                            tracing::error!("Dispatch error: {}", error);
                        }
                    }
                    Err(error) => tracing::error!("Intake error: {}", error),
                },
                TransferType::Ping => {
                    tracing::debug!("Ping from {}", connection.peer_addr());
                }
                other => {
                    tracing::warn!("Unexpected packet type: {:?}", other);
                }
            },
            Err(error) => {
                tracing::info!("Client disconnected: {}", error);
                break;
            }
        }
    }

    let _ = audit.log_session_end(&session.id, transfer_count);
}
