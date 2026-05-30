use adtp_core::{
    encryption::decrypt_file_bytes,
    errors::{AdtpError, AdtpResult},
    integrity::verify_hash,
    protocol::{AdtpPacket, ContextEnvelope},
    types::AES_KEY_SIZE,
};
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct FileReceiver {
    receive_dir: PathBuf,
}

impl FileReceiver {
    pub fn new(receive_dir: PathBuf) -> Self {
        Self { receive_dir }
    }

    pub async fn handle_push(
        &self,
        packet: &AdtpPacket,
        session_key: &[u8; AES_KEY_SIZE],
    ) -> AdtpResult<PathBuf> {
        let envelope: ContextEnvelope =
            serde_json::from_slice(&packet.context).map_err(AdtpError::Json)?;
        let filename = &envelope.file_meta.filename;

        tracing::info!("Receiving file: {}", filename);

        let plaintext = decrypt_file_bytes(session_key, &packet.header.nonce, &packet.payload)?;
        verify_hash(&plaintext, &packet.header.content_hash)?;

        fs::create_dir_all(&self.receive_dir).await?;
        let dest_path = self.receive_dir.join(filename);
        fs::write(&dest_path, &plaintext).await?;

        tracing::info!("Saved to: {}", dest_path.display());
        Self::notify(filename, &dest_path);

        Ok(dest_path)
    }

    fn notify(filename: &str, path: &Path) {
        #[cfg(not(test))]
        {
            let _ = notify_rust::Notification::new()
                .summary("ADTP — File Received")
                .body(&format!("{} saved to {}.", filename, path.display()))
                .icon("document-open")
                .show();
        }

        #[cfg(test)]
        let _ = (filename, path);
    }
}
