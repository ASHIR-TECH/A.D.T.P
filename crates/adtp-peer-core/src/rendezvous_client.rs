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
    ) -> AdtpResult<()> {
        let local_addr = self
            .socket
            .local_addr()
            .map_err(|e| AdtpError::RendezvousError {
                reason: format!("local addr: {e}"),
            })?;

        let payload = PeerRegisterPayload {
            peer_id: *peer_id,
            external_addr: local_addr.to_string(),
            nat_type,
            supports_ipv6,
            can_relay,
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
