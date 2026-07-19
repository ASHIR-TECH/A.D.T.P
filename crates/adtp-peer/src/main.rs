mod cli_args;

use adtp_core::{
    session::new_session,
    types::{TransferRecord, TransferStatus, TransferType, AES_KEY_SIZE},
};
use adtp_peer_core::{
    agent_card::{build_unsigned_card, sign_card, UnsignedCardInput},
    agent_card_server::AgentCardServer,
    audit::AuditLog,
    dual_endpoint::DualEndpoint,
    identity::PeerIdentityKey,
    intake::FileIntake,
    trust::TrustStore,
};
use chrono::Utc;
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_env("ADTP_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = cli_args::CliArgs::parse();

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("adtp");

    let identity_path = config_dir.join("identity.key");
    let identity = match PeerIdentityKey::load_or_generate(&identity_path) {
        Ok(id) => {
            tracing::info!(
                "Peer identity loaded — peer_id: {}",
                hex::encode(id.peer_id)
            );
            id
        }
        Err(e) => {
            eprintln!("Failed to load/generate identity: {e}");
            std::process::exit(1);
        }
    };

    println!("peer_id: {}", hex::encode(identity.peer_id));

    let unsigned_card = build_unsigned_card(UnsignedCardInput {
        peer_id: &identity.peer_id,
        public_key: &identity.verifying_key.to_bytes(),
        display_name: None,
        host: &args.listen_host,
        port: args.listen_port,
        max_file_size: adtp_core::types::ADTP_MAX_FILE_SIZE,
        supported_mime_types: vec![],
        supports_tasks: true,
        supports_multihop: true,
        relay_of: vec![],
    });
    let agent_card = sign_card(&unsigned_card, &identity.signing_key);
    let card_server = AgentCardServer::new(args.agent_card_port, agent_card);
    tokio::spawn(async move {
        card_server.serve().await;
    });
    tracing::info!(
        "Agent Card server starting on port {}",
        args.agent_card_port
    );

    let peers_path = config_dir.join("peers.toml");
    let trust_store = TrustStore::load(&peers_path);

    let addr: SocketAddr = format!("{}:{}", args.listen_host, args.listen_port)
        .parse()
        .expect("Invalid listen address");

    let endpoint = Arc::new(DualEndpoint::bind(addr).await.unwrap_or_else(|e| {
        eprintln!("Failed to bind dual endpoint: {e}");
        std::process::exit(1);
    }));

    let audit = Arc::new(
        AuditLog::open(&config_dir.join("peer-audit.db")).unwrap_or_else(|e| {
            eprintln!("Failed to open audit log: {e}");
            std::process::exit(1);
        }),
    );

    let intake = Arc::new(FileIntake::new(
        args.intake_dir
            .clone()
            .unwrap_or_else(|| config_dir.join("intake")),
    ));

    tracing::info!("ADTP peer listening on {}", addr);

    let accept_audit = audit.clone();
    let accept_intake = intake.clone();
    let accept_trust = trust_store;

    loop {
        match endpoint.accept().await {
            Ok(Some(connection)) => {
                let peer_addr = connection.peer_addr();
                let session = new_session(&peer_addr);

                accept_audit
                    .log_session_start(&session.id, &peer_addr)
                    .unwrap_or_else(|e| tracing::error!("Audit session start failed: {e}"));

                let session_key = [0u8; AES_KEY_SIZE];
                let mut transfer_count = 0usize;

                loop {
                    match connection.recv_packet().await {
                        Ok(packet) => match packet.header.transfer_type {
                            TransferType::Transfer => {
                                let sender_id = packet.header.sender_peer_id;
                                if !accept_trust.is_allowed(&sender_id) {
                                    tracing::warn!(
                                        "Transfer from untrusted peer: {}",
                                        hex::encode(sender_id)
                                    );
                                    continue;
                                }
                                match accept_intake.process(&packet, &session_key).await {
                                    Ok(result) => {
                                        transfer_count += 1;
                                        let record = TransferRecord {
                                            id: packet.header.transfer_id,
                                            session_id: session.id,
                                            sender_peer_id: sender_id,
                                            receiver_peer_id: identity.peer_id,
                                            transfer_type: TransferType::Transfer,
                                            file_meta: result.file_meta.clone(),
                                            status: TransferStatus::Delivered,
                                            initiated_at: Utc::now(),
                                            completed_at: Some(Utc::now()),
                                            signature: hex::encode(packet.signature),
                                            received_hash: Some(packet.header.content_hash),
                                            integrity_verified: true,
                                        };
                                        if let Err(e) = accept_audit.log_transfer(&record) {
                                            tracing::error!("Audit error: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Intake error: {}", e);
                                    }
                                }
                            }
                            TransferType::Hello => {
                                tracing::info!("Hello from peer {}", connection.peer_addr());
                            }
                            TransferType::Ping => {
                                tracing::debug!("Ping from {}", connection.peer_addr());
                            }
                            other => {
                                tracing::warn!("Unexpected packet type: {:?}", other);
                            }
                        },
                        Err(e) => {
                            tracing::info!("Peer disconnected: {}", e);
                            break;
                        }
                    }
                }

                let _ = accept_audit.log_session_end(&session.id, transfer_count);
            }
            Ok(None) => break,
            Err(e) => {
                tracing::error!("Accept error: {}", e);
                break;
            }
        }
    }
}
