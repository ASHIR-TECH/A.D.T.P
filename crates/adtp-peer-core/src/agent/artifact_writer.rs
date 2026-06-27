use chrono::Utc;

pub struct ArtifactWriter {
    output_dir: Option<std::path::PathBuf>,
}

impl ArtifactWriter {
    pub fn new() -> Self {
        let output_dir = dirs::data_local_dir().map(|p| p.join("adtp").join("artifacts"));
        if let Some(ref dir) = output_dir {
            let _ = std::fs::create_dir_all(dir);
        }
        Self { output_dir }
    }

    pub fn new_with_dir(dir: std::path::PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self { output_dir: Some(dir) }
    }

    pub async fn write_artifact(&self, content: &str) -> Result<String, String> {
        let dir = self.output_dir.as_ref().ok_or("Cannot determine data directory")?;
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S_%3f");
        let filename = format!("artifact_{}.txt", timestamp);
        let path = dir.join(&filename);
        std::fs::write(&path, content).map_err(|e| format!("Failed to write artifact: {}", e))?;
        Ok(path.to_string_lossy().to_string())
    }

    pub async fn write_artifact_named(&self, name: &str, content: &str) -> Result<String, String> {
        let dir = self.output_dir.as_ref().ok_or("Cannot determine data directory")?;
        let safe_name = name.replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-', "_");
        let path = dir.join(&safe_name);
        std::fs::write(&path, content).map_err(|e| format!("Failed to write artifact: {}", e))?;
        Ok(path.to_string_lossy().to_string())
    }
}
