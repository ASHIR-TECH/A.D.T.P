use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub artifact_id: Uuid,
    pub task_id: Uuid,
    pub transfer_id: Uuid,
    pub artifact_name: String,
    pub mime_type: String,
    pub file_path: String,
    pub file_size: u64,
    pub content_hash: [u8; 32],
    pub produced_at: DateTime<Utc>,
}

impl Artifact {
    pub fn new(
        task_id: Uuid,
        transfer_id: Uuid,
        artifact_name: String,
        mime_type: String,
        file_path: String,
        file_size: u64,
        content_hash: [u8; 32],
    ) -> Self {
        Self {
            artifact_id: Uuid::new_v4(),
            task_id,
            transfer_id,
            artifact_name,
            mime_type,
            file_path,
            file_size,
            content_hash,
            produced_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactResponse {
    pub artifact_id: String,
    pub task_id: String,
    pub transfer_id: String,
    pub artifact_name: String,
    pub mime_type: String,
    pub file_path: String,
    pub file_size: u64,
    pub produced_at: String,
}

impl From<Artifact> for ArtifactResponse {
    fn from(a: Artifact) -> Self {
        Self {
            artifact_id: a.artifact_id.to_string(),
            task_id: a.task_id.to_string(),
            transfer_id: a.transfer_id.to_string(),
            artifact_name: a.artifact_name,
            mime_type: a.mime_type,
            file_path: a.file_path,
            file_size: a.file_size,
            produced_at: a.produced_at.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_creation() {
        let artifact = Artifact::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "report.pdf".into(),
            "application/pdf".into(),
            "/tmp/report.pdf".into(),
            2048,
            [0u8; 32],
        );
        assert_eq!(artifact.artifact_name, "report.pdf");
        assert_eq!(artifact.file_size, 2048);
        assert_eq!(artifact.mime_type, "application/pdf");
    }

    #[test]
    fn test_artifact_response_conversion() {
        let artifact = Artifact::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "output.json".into(),
            "application/json".into(),
            "/tmp/output.json".into(),
            512,
            [1u8; 32],
        );
        let response: ArtifactResponse = artifact.into();
        assert_eq!(response.artifact_name, "output.json");
        assert_eq!(response.file_size, 512);
    }
}
