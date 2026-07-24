use adtp_core::errors::{AdtpError, AdtpResult};
use adtp_core::types::{NatType, PeerRegisterPayload, PEER_ID_SIZE};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub struct RendezvousClient {
    socket: UdpSocket,
    server_addr: SocketAddr,
}

impl RendezvousClient {
    pub async fn connect(server_host: &str, server_port: u16) -> AdtpResult<Self> {
        let socket =
            UdpSocket::bind("0.0.0.0:0")
                .await
                .map_err(|e| AdtpError::RendezvousError {
                    reason: format!("bind: {e}"),
                })?;

        let server_addr: SocketAddr =
            format!("{server_host}:{server_port}")
                .parse()
                .map_err(|e| AdtpError::RendezvousError {
                    reason: format!("invalid server addr: {e}"),
                })?;

        Ok(Self {
            socket,
            server_addr,
        })
    }

    pub async fn register(
        &self,
        peer_id: &[u8; PEER_ID_SIZE],
        nat_type: NatType,
        supports_ipv6: bool,
        can_relay: bool,
        signing_key: Option<&ed25519_dalek::SigningKey>,
    ) -> AdtpResult<()> {
        let local_addr = self
            .socket
            .local_addr()
            .map_err(|e| AdtpError::RendezvousError {
                reason: format!("local addr: {e}"),
            })?;

        let (public_key_hex, signature_hex, timestamp) = if let Some(key) = signing_key {
            let now = chrono::Utc::now().timestamp_millis();
            let verifying_key = key.verifying_key();
            let pk_hex = hex::encode(verifying_key.to_bytes());

            let mut msg = Vec::new();
            msg.extend_from_slice(peer_id);
            msg.extend_from_slice(local_addr.to_string().as_bytes());
            msg.extend_from_slice(&now.to_le_bytes());

            use ed25519_dalek::Signer;
            let sig = key.sign(&msg);
            let sig_hex = hex::encode(sig.to_bytes());

            (Some(pk_hex), Some(sig_hex), Some(now))
        } else {
            (None, None, None)
        };

        let payload = PeerRegisterPayload {
            peer_id: *peer_id,
            external_addr: local_addr.to_string(),
            nat_type,
            supports_ipv6,
            can_relay,
            public_key: public_key_hex,
            signature: signature_hex,
            timestamp,
        };

        let mut buf = vec![0x20u8];
        buf.extend(
            serde_json::to_vec(&payload).map_err(|e| AdtpError::RendezvousError {
                reason: format!("serialize: {e}"),
            })?,
        );

        self.socket
            .send_to(&buf, self.server_addr)
            .await
            .map_err(|e| AdtpError::RendezvousError {
                reason: format!("send register: {e}"),
            })?;

        Ok(())
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    pub fn server_addr(&self) -> &SocketAddr {
        &self.server_addr
    }
}
