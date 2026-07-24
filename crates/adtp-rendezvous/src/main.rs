mod peer_registry;
mod peer_table;
mod stun;
mod token;
mod wake;

use adtp_core::types::{
    PeerRegisterPayload, RendezvousResponsePayload, RENDEZVOUS_DEFAULT_PORT, TOKEN_VALIDITY_SECS,
};
use peer_registry::PeerRegistry;
use peer_table::PeerTable;
use std::net::SocketAddr;
use std::sync::Arc;
use stun::StunServer;
use token::TokenIssuer;
use tokio::net::UdpSocket;
use tracing_subscriber::EnvFilter;
use wake::WakeService;

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_env("ADTP_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let port: u16 = std::env::var("ADTP_RENDEZVOUS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(RENDEZVOUS_DEFAULT_PORT);

    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("Invalid bind address");
    let socket = Arc::new(UdpSocket::bind(addr).await.unwrap_or_else(|e| {
        eprintln!("Failed to bind rendezvous server on {addr}: {e}");
        std::process::exit(1);
    }));

    tracing::info!("Rendezvous server listening on UDP {port}");

    let peer_table = Arc::new(PeerTable::new());
    let token_issuer = Arc::new(TokenIssuer::new());
    let wake_service = Arc::new(WakeService::new());
    let stun_server = Arc::new(StunServer);
    let peer_registry = Arc::new(PeerRegistry::new());

    let mut buf = [0u8; 4096];

    loop {
        let (len, peer_addr) = match socket.recv_from(&mut buf).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("recv error: {e}");
                continue;
            }
        };

        let data = buf[..len].to_vec();
        let socket = socket.clone();
        let peer_table = peer_table.clone();
        let token_issuer = token_issuer.clone();
        let wake_service = wake_service.clone();
        let stun_server = stun_server.clone();
        let peer_registry = peer_registry.clone();

        tokio::spawn(async move {
            handle_packet(
                &data,
                peer_addr,
                &socket,
                &peer_table,
                &token_issuer,
                &wake_service,
                &stun_server,
                &peer_registry,
            )
            .await;
        });
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_packet(
    data: &[u8],
    peer_addr: SocketAddr,
    socket: &UdpSocket,
    peer_table: &PeerTable,
    token_issuer: &TokenIssuer,
    wake_service: &WakeService,
    stun_server: &StunServer,
    peer_registry: &PeerRegistry,
) {
    if data.len() < 2 {
        return;
    }

    let packet_type = data[0];

    match packet_type {
        0x20 => {
            if let Ok(payload) = serde_json::from_slice::<PeerRegisterPayload>(&data[1..]) {
                if let (Some(pub_key_hex), Some(sig_hex), Some(ts)) = (
                    &payload.public_key,
                    &payload.signature,
                    &payload.timestamp,
                ) {
                    let now = chrono::Utc::now().timestamp_millis();
                    if (now - ts).abs() > 60_000 {
                        tracing::warn!(
                            "Registration timestamp too old/future from {}",
                            hex::encode(payload.peer_id)
                        );
                        return;
                    }
                    if let Ok(pub_key_bytes) = hex::decode(pub_key_hex) {
                        if pub_key_bytes.len() == 32 {
                            let mut pk = [0u8; 32];
                            pk.copy_from_slice(&pub_key_bytes);

                            let mut msg = Vec::new();
                            msg.extend_from_slice(&payload.peer_id);
                            msg.extend_from_slice(payload.external_addr.as_bytes());
                            msg.extend_from_slice(&ts.to_le_bytes());
                            if !peer_registry.verify_signature(
                                &payload.peer_id,
                                sig_hex,
                                &msg,
                            ) {
                                tracing::warn!(
                                    "Invalid registration signature from {}",
                                    hex::encode(payload.peer_id)
                                );
                                return;
                            }
                            peer_registry.register(&payload.peer_id, &pk);
                        }
                    }
                }

                let reflected_addr = stun_server.reflect_external_addr(&peer_addr);
                let external_addr = if payload.external_addr.trim().is_empty() {
                    reflected_addr
                } else {
                    payload.external_addr
                };
                peer_table.register(
                    payload.peer_id,
                    external_addr.clone(),
                    payload.nat_type,
                    payload.supports_ipv6,
                    payload.can_relay,
                );
                tracing::info!(
                    "Peer registered: {} at {} (nat={:?}, ipv6={}, relay={})",
                    hex::encode(payload.peer_id),
                    external_addr,
                    payload.nat_type,
                    payload.supports_ipv6,
                    payload.can_relay
                );
            }
        }
        0x21 => {
            if let Ok(reg) =
                serde_json::from_slice::<adtp_core::types::RendezvousRequestPayload>(&data[1..])
            {
                if let Some(token) = &reg.token {
                    if !token_issuer.validate(token) || !token_issuer.consume(token) {
                        tracing::warn!(
                            "Rejected consumed or invalid rendezvous token from {}",
                            hex::encode(reg.requester_peer_id)
                        );
                        return;
                    }
                }
                peer_table.refresh(&reg.requester_peer_id);
                let target_info = peer_table.lookup(&reg.target_peer_id);
                match target_info {
                    Some(info) => {
                        let token = token_issuer.issue(&reg.requester_peer_id, &reg.target_peer_id);
                        let expires_at =
                            chrono::Utc::now().timestamp() as u64 + TOKEN_VALIDITY_SECS;
                        let resp = RendezvousResponsePayload {
                            token: token.clone(),
                            target_external_addr: info.external_addr.clone(),
                            target_peer_id: reg.target_peer_id,
                            expires_at,
                        };
                        let mut resp_bytes = vec![0x22u8];
                        if let Ok(json) = serde_json::to_vec(&resp) {
                            resp_bytes.extend(json);
                            let _ = socket.send_to(&resp_bytes, peer_addr).await;
                        }
                        tracing::debug!(
                            "Rendezvous response for target {} (nat={:?}, ipv6={}, relay={})",
                            hex::encode(reg.target_peer_id),
                            info.nat_type,
                            info.supports_ipv6,
                            info.can_relay
                        );

                        let wake_bytes = wake_service.build_wake(&reg.requester_peer_id, &token);
                        if let Some(target_addr) = peer_table.addr(&reg.target_peer_id) {
                            let _ = socket.send_to(&wake_bytes, target_addr).await;
                        }
                    }
                    None => {
                        tracing::warn!(
                            "Target peer not found: {}",
                            hex::encode(reg.target_peer_id)
                        );
                    }
                }
            }
        }
        0x22 | 0x23 => {
            // PunchReady forwarded to target
            let target_addr = peer_table.addr_for_punch(data);
            if let Some(addr) = target_addr {
                let _ = socket.send_to(data, addr).await;
            }
        }
        _ => {
            tracing::debug!("Unknown packet type: {packet_type:#04x} from {peer_addr}");
        }
    }
}
