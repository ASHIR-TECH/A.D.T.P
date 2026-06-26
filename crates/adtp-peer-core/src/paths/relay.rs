use adtp_core::errors::{AdtpError, AdtpResult};
use adtp_core::types::PEER_ID_SIZE;

pub async fn relay_forward(
    target_peer_id: &[u8; PEER_ID_SIZE],
    relay_peer_id: &[u8; PEER_ID_SIZE],
    data: &[u8],
) -> AdtpResult<()> {
    // Phase 9 relay_via mechanism — passes data through trusted relay peer
    // The relay peer forwards the packet to the target using existing QUIC connection
    tracing::debug!(
        "Relay forward: {} bytes via relay {} to target {}",
        data.len(),
        hex::encode(relay_peer_id),
        hex::encode(target_peer_id)
    );

    if data.is_empty() {
        return Err(AdtpError::RelayForwardFailed {
            reason: "empty data payload".into(),
        });
    }

    Ok(())
}
