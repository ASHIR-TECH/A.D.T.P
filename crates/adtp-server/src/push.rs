use adtp_core::{
    encryption::encrypt_file_bytes,
    errors::AdtpResult,
    integrity::hash_file,
    protocol::{AdtpPacket, ContextEnvelope, PacketHeader, PacketHeaderParts},
    transfer::AdtpConnection,
    types::*,
};
use bytes::Bytes;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use uuid::Uuid;

pub struct PushHandler {
    connection: Arc<AdtpConnection>,
    session_id: Uuid,
    session_key: [u8; AES_KEY_SIZE],
}

impl PushHandler {
    pub fn new(
        connection: Arc<AdtpConnection>,
        session_id: Uuid,
        session_key: [u8; AES_KEY_SIZE],
    ) -> Self {
        Self {
            connection,
            session_id,
            session_key,
        }
    }

    pub async fn push(&self, file_path: &PathBuf) -> AdtpResult<()> {
        let transfer_id = Uuid::new_v4();
        let content_hash = hash_file(file_path).await?;
        let file_bytes = fs::read(file_path).await?;
        let mime_type = infer::get(&file_bytes)
            .map(|kind| kind.mime_type().to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let filename = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let file_meta = FileMeta {
            filename: filename.clone(),
            mime_type,
            file_size: file_bytes.len() as u64,
            extension: file_path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase(),
            last_modified: None,
            content_hash,
        };
        let context_env = ContextEnvelope {
            file_meta,
            context: ContextBlock::default(),
        };
        let context_bytes = serde_json::to_vec(&context_env)?;
        let (ciphertext, nonce) = encrypt_file_bytes(&self.session_key, &file_bytes)?;
        let header = PacketHeader::new(PacketHeaderParts {
            transfer_type: TransferType::Transfer,
            session_id: self.session_id,
            transfer_id,
            sender_peer_id: [0u8; PEER_ID_SIZE],
            receiver_peer_id: [0u8; PEER_ID_SIZE],
            nonce,
            content_hash,
            context_len: context_bytes.len() as u32,
            payload_len: ciphertext.len() as u64,
        });
        let packet = AdtpPacket {
            header,
            context: Bytes::from(context_bytes),
            payload: Bytes::from(ciphertext),
            signature: [0u8; ED25519_SIG_SIZE],
        };

        self.connection.send_packet(&packet).await?;
        tracing::info!("Pushed: {} to {}", filename, self.connection.peer_addr());

        Ok(())
    }
}
