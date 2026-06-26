use adtp_core::errors::{AdtpError, AdtpResult};
use adtp_core::types::PEER_ID_SIZE;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;

pub async fn dial_direct(target: &SocketAddr, _peer_id: &[u8; PEER_ID_SIZE]) -> AdtpResult<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| AdtpError::QuicConnectionFailed {
        reason: format!("bind failed: {e}"),
    })?;

    // Quick connectivity check via UDP
    let mut probe = vec![0u8; 1];
    probe[0] = 0x0A; // Ping
    socket.send_to(&probe, target).await.map_err(|e| AdtpError::QuicConnectionFailed {
        reason: format!("send probe to {target} failed: {e}"),
    })?;

    let mut buf = [0u8; 1];
    tokio::time::timeout(Duration::from_millis(500), socket.recv_from(&mut buf)).await
        .map_err(|_| AdtpError::QuicConnectionFailed {
            reason: format!("direct dial timeout to {target}"),
        })?
        .map_err(|e| AdtpError::QuicConnectionFailed {
            reason: format!("recv failed: {e}"),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dial_direct_invalid_addr() {
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let result = dial_direct(&addr, &[0u8; PEER_ID_SIZE]).await;
        // Should fail since nothing is listening on port 1
        assert!(result.is_err());
    }
}
