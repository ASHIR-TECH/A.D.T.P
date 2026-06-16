use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub api_url: String,
    pub api_token: String,
    pub default_agent: String,
    pub output_format: String,
    pub poll_interval_ms: u64,
    pub connect_timeout_ms: u64,
    pub request_timeout_ms: u64,

    #[serde(default)]
    pub agents: Vec<AgentEntry>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            api_url: "http://127.0.0.1:40003".into(),
            api_token: String::new(),
            default_agent: "default".into(),
            output_format: "table".into(),
            poll_interval_ms: 500,
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            agents: Vec::new(),
        }
    }
}

impl CliConfig {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("adtp")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("cli.toml")
    }

    pub fn load() -> Result<Self, crate::errors::CliError> {
        let path = Self::config_path();
        let mut cfg = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            toml::from_str(&content)?
        } else {
            Self::default()
        };
        cfg.apply_env_overrides();
        Ok(cfg)
    }

    pub fn save(&self) -> Result<(), crate::errors::CliError> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(self).map_err(|e| {
            crate::errors::CliError::Parse { message: e.to_string() }
        })?;
        std::fs::write(Self::config_path(), content)?;
        Ok(())
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("ADTP_API_URL") {
            self.api_url = v;
        }
        if let Ok(v) = std::env::var("ADTP_API_TOKEN") {
            self.api_token = v;
        }
        if let Ok(v) = std::env::var("ADTP_DEFAULT_AGENT") {
            self.default_agent = v;
        }
        if let Ok(v) = std::env::var("ADTP_OUTPUT_FORMAT") {
            self.output_format = v;
        }
        if let Ok(v) = std::env::var("ADTP_POLL_INTERVAL") {
            if let Ok(n) = v.parse() {
                self.poll_interval_ms = n;
            }
        }
        if let Ok(v) = std::env::var("ADTP_CONNECT_TIMEOUT") {
            if let Ok(n) = v.parse() {
                self.connect_timeout_ms = n;
            }
        }
        if let Ok(v) = std::env::var("ADTP_REQUEST_TIMEOUT") {
            if let Ok(n) = v.parse() {
                self.request_timeout_ms = n;
            }
        }
    }

    pub fn validate(&self) -> Result<(), crate::errors::CliError> {
        if self.api_token.is_empty() {
            return Err(crate::errors::CliError::Config {
                message: "ADTP_API_TOKEN is not set. Set it in cli.toml or ADTP_API_TOKEN env var.".into(),
            });
        }
        Ok(())
    }
}
