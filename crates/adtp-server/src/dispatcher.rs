use crate::intake::IntakeResult;
use adtp_core::errors::AdtpResult;
use std::path::PathBuf;
use tokio::fs;

pub struct Dispatcher {
    output_dir: PathBuf,
}

impl Dispatcher {
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    pub async fn dispatch(&self, intake: &IntakeResult) -> AdtpResult<()> {
        fs::create_dir_all(&self.output_dir).await?;
        let manifest = serde_json::json!({
            "event": "file_received",
            "file_path": intake.file_path.display().to_string(),
            "filename": intake.file_meta.filename,
            "mime_type": intake.file_meta.mime_type,
            "file_size": intake.file_meta.file_size,
            "context": intake.context_env.context,
        });
        let manifest_path = self
            .output_dir
            .join(format!("{}.manifest.json", intake.file_meta.filename));

        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).await?;
        tracing::info!("Dispatched manifest: {}", manifest_path.display());

        Ok(())
    }
}
