use adtp_core::{
    encryption::decrypt_file_bytes,
    errors::{AdtpError, AdtpResult},
    integrity::verify_hash,
    protocol::{AdtpPacket, ContextEnvelope},
    types::{FileMeta, ADTP_MAX_FILE_SIZE, AES_KEY_SIZE},
};
use std::path::PathBuf;
use tokio::fs;

pub struct IntakeResult {
    pub file_path: PathBuf,
    pub file_meta: FileMeta,
    pub context_env: ContextEnvelope,
}

pub struct FileIntake {
    intake_dir: PathBuf,
}

impl FileIntake {
    pub fn new(intake_dir: PathBuf) -> Self {
        Self { intake_dir }
    }

    pub async fn process(
        &self,
        packet: &AdtpPacket,
        session_key: &[u8; AES_KEY_SIZE],
    ) -> AdtpResult<IntakeResult> {
        let context_env: ContextEnvelope =
            serde_json::from_slice(&packet.context).map_err(AdtpError::Json)?;
        let filename = &context_env.file_meta.filename;
        let file_size = context_env.file_meta.file_size;

        tracing::info!("Intake: {} ({} bytes)", filename, file_size);

        if file_size > ADTP_MAX_FILE_SIZE {
            return Err(AdtpError::FileTooLarge {
                size: file_size,
                limit: ADTP_MAX_FILE_SIZE,
            });
        }

        let plaintext = decrypt_file_bytes(session_key, &packet.header.nonce, &packet.payload)?;
        verify_hash(&plaintext, &packet.header.content_hash)?;

        fs::create_dir_all(&self.intake_dir).await?;
        let dest = self.intake_dir.join(filename);
        fs::write(&dest, &plaintext).await?;

        tracing::info!("Written to intake: {}", dest.display());

        Ok(IntakeResult {
            file_path: dest,
            file_meta: context_env.file_meta.clone(),
            context_env,
        })
    }
}
