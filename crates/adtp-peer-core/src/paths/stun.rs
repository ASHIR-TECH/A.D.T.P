use adtp_core::errors::{AdtpError, AdtpResult};
use adtp_core::types::{PunchReadyPayload, RendezvousRequestPayload, RendezvousResponsePayload, PEER_ID_SIZE};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

pub struct StunPunch {
    socket: UdpSocket,
}

impl StunPunch {
    pub async fn new() -> AdtpResult<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| AdtpError::HolePunchFailed {
            reason: format!("bind: {e}"),
        })?;
        Ok(Self { socket })
    }

    pub async fn request_punch(
        &self,
        rendezvous_addr: &SocketAddr,
        requester: &[u8; PEER_ID_SIZE],
        target: &[u8; PEER_ID_SIZE],
    ) -> AdtpResult<RendezvousResponsePayload> {
        let req = RendezvousRequestPayload {
            requester_peer_id: *requester,
            target_peer_id: *target,
            token: None,
        };
        let mut buf = vec![0x21u8];
        buf.extend(serde_json::to_vec(&req).map_err(|e| AdtpError::HolePunchFailed {
            reason: format!("serialize: {e}"),
        })?);
        self.socket.send_to(&buf, rendezvous_addr).await.map_err(|e| AdtpError::HolePunchFailed {
            reason: format!("send rendezvous request: {e}"),
        })?;

        let mut recv_buf = [0u8; 2048];
        timeout(Duration::from_millis(1500), self.socket.recv_from(&mut recv_buf)).await
            .map_err(|_| AdtpError::HolePunchFailed {
                reason: "rendezvous response timeout".into(),
            })?
            .map_err(|e| AdtpError::HolePunchFailed {
                reason: format!("recv: {e}"),
            })?;

        if recv_buf[0] != 0x22 {
            return Err(AdtpError::HolePunchFailed {
                reason: "unexpected response type".into(),
            });
        }

        let resp: RendezvousResponsePayload = serde_json::from_slice(&recv_buf[1..]).map_err(|e| AdtpError::HolePunchFailed {
            reason: format!("deserialize: {e}"),
        })?;

        Ok(resp)
    }

    pub async fn punch(
        &self,
        token: &str,
        sender_id: &[u8; PEER_ID_SIZE],
        target_external: &SocketAddr,
    ) -> AdtpResult<()> {
        let payload = PunchReadyPayload {
            token: token.to_string(),
            sender_peer_id: *sender_id,
        };
        let mut buf = vec![0x23u8];
        buf.extend(serde_json::to_vec(&payload).map_err(|e| AdtpError::HolePunchFailed {
            reason: format!("serialize punch: {e}"),
        })?);

        // Send multiple punch packets
        for _ in 0..3 {
            let _ = self.socket.send_to(&buf, target_external).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Listen for incoming punch response
        let mut recv_buf = [0u8; 2048];
        timeout(Duration::from_millis(1500), self.socket.recv_from(&mut recv_buf)).await
            .map_err(|_| AdtpError::HolePunchFailed {
                reason: "punch response timeout".into(),
            })?
            .map_err(|e| AdtpError::HolePunchFailed {
                reason: format!("recv punch: {e}"),
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stun_punch_new() {
        let punch = StunPunch::new().await;
        assert!(punch.is_ok());
    }
}
