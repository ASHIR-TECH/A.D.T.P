use adtp_core::errors::{AdtpError, AdtpResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub receive_dir: PathBuf,
    pub keys_dir: PathBuf,
    pub agents: Vec<AgentConfig>,
    pub audit_db: PathBuf,
    pub max_file_size: u64,
    pub connect_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub default: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            receive_dir: home.join("adtp-received"),
            keys_dir: home.join(".config/adtp/keys"),
            agents: Vec::new(),
            audit_db: home.join(".config/adtp/client-audit.db"),
            max_file_size: adtp_core::types::ADTP_MAX_FILE_SIZE,
            connect_timeout_secs: 30,
        }
    }
}

impl ClientConfig {
    pub fn load() -> AdtpResult<Self> {
        let config_path = Self::config_path();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content).map_err(|e| AdtpError::Internal {
                reason: e.to_string(),
            })
        } else {
            let config = ClientConfig::default();
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
            .unwrap_or_else(|| PathBuf::from("."))
            .join("adtp/client.toml")
    }
}
