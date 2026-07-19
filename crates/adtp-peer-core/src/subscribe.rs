use adtp_core::errors::{AdtpError, AdtpResult};
use adtp_core::protocol::{AdtpPacket, PacketHeaderParts};
use adtp_core::transfer::AdtpConnection;
use adtp_core::types::TransferType;
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct SubscribeManager {
    connections: Mutex<Vec<SubscribePeer>>,
}

struct SubscribePeer {
    peer_id: [u8; 16],
    connection: Arc<AdtpConnection>,
    sequence_number: u64,
}

impl SubscribeManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(Vec::new()),
        }
    }

    pub async fn register(&self, peer_id: [u8; 16], connection: Arc<AdtpConnection>) {
        let mut conns = self.connections.lock().await;
        conns.retain(|p| p.peer_id != peer_id);
        conns.push(SubscribePeer {
            peer_id,
            connection,
            sequence_number: 0,
        });
        tracing::info!(
            "Subscribe stream registered for peer {}",
            hex::encode(peer_id)
        );
    }

    pub async fn unregister(&self, peer_id: &[u8; 16]) {
        let mut conns = self.connections.lock().await;
        conns.retain(|p| p.peer_id != *peer_id);
    }

    pub async fn send_notify(
        &self,
        target_peer: &[u8; 16],
        transfer_id: Uuid,
        notify_type: NotifyType,
    ) -> AdtpResult<()> {
        let mut conns = self.connections.lock().await;
        let peer = match conns.iter_mut().find(|p| p.peer_id == *target_peer) {
            Some(p) => p,
            None => {
                tracing::warn!(
                    "No subscribe stream for peer {}, cannot send notify",
                    hex::encode(target_peer)
                );
                return Err(AdtpError::Internal {
                    reason: format!("No subscribe stream for peer {}", hex::encode(target_peer)),
                });
            }
        };

        peer.sequence_number += 1;

        let notify_payload = NotifyPayload {
            sequence: peer.sequence_number,
            transfer_id,
            notify_type,
        };
        let context_bytes = serde_json::to_vec(&notify_payload)?;

        let packet = AdtpPacket {
            header: adtp_core::protocol::PacketHeader::new(PacketHeaderParts {
                transfer_type: TransferType::Notify,
                session_id: Uuid::nil(),
                transfer_id: Uuid::new_v4(),
                sender_peer_id: [0u8; 16],
                receiver_peer_id: *target_peer,
                nonce: [0u8; 12],
                content_hash: [0u8; 32],
                context_len: context_bytes.len() as u32,
                payload_len: 0,
            }),
            context: Bytes::from(context_bytes),
            payload: Bytes::new(),
            signature: [0u8; 64],
        };

        peer.connection.send_packet(&packet).await?;
        tracing::debug!(
            "Notify sent to peer {} (seq {})",
            hex::encode(target_peer),
            peer.sequence_number
        );
        Ok(())
    }

    pub async fn connected_peers(&self) -> Vec<[u8; 16]> {
        let conns = self.connections.lock().await;
        conns.iter().map(|p| p.peer_id).collect()
    }
}

impl Default for SubscribeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NotifyPayload {
    pub sequence: u64,
    pub transfer_id: Uuid,
    pub notify_type: NotifyType,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NotifyType {
    NewTransfer,
    TaskUpdate,
    ArtifactReady,
    Disconnect,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubscribeRequest {
    pub peer_id: [u8; 16],
    pub last_sequence: u64,
}
