use adtp_core::errors::{AdtpError, AdtpResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub port: u16,
    pub intake_dir: PathBuf,
    pub output_dir: PathBuf,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
    pub audit_db: PathBuf,
    pub max_connections: usize,
    pub max_file_size: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        let data = dirs::data_dir().unwrap_or_else(|| PathBuf::from("/var/lib/adtp"));
        Self {
            bind_addr: "0.0.0.0".to_string(),
            port: adtp_core::types::ADTP_DEFAULT_PORT,
            intake_dir: data.join("adtp/intake"),
            output_dir: data.join("adtp/output"),
            tls_cert: None,
            tls_key: None,
            audit_db: data.join("adtp/server-audit.db"),
            max_connections: 100,
            max_file_size: adtp_core::types::ADTP_MAX_FILE_SIZE,
        }
    }
}

impl ServerConfig {
    pub fn load() -> AdtpResult<Self> {
        let config_path = Self::config_path();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content).map_err(|e| AdtpError::Internal {
                reason: e.to_string(),
            })
        } else {
            let config = ServerConfig::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> AdtpResult<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| AdtpError::Internal {
            reason: e.to_string(),
        })?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/etc/adtp"))
            .join("adtp/server.toml")
    }
}
