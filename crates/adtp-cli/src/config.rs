use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

    #[serde(default)]
    pub default_agent: String,

    #[serde(default)]
    pub default_peer: Option<String>,

    #[serde(default)]
    pub output_format: String,

    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,

    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_ms: u64,

    #[serde(default = "default_request_timeout")]
    pub request_timeout_ms: u64,

    #[serde(default = "default_keepalive")]
    pub keepalive_ms: u64,

    #[serde(default)]
    pub nat_override: Option<String>,

    #[serde(default = "default_rendezvous")]
    pub rendezvous_server: String,

    #[serde(default)]
    pub agents: Vec<AgentEntry>,
}

fn default_poll_interval() -> u64 {
    500
}
fn default_connect_timeout() -> u64 {
    5000
}
fn default_request_timeout() -> u64 {
    30000
}
fn default_keepalive() -> u64 {
    25000
}
fn default_rendezvous() -> String {
    "udp://ashir.world:40004".into()
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            api_url: "http://127.0.0.1:40003".into(),
            api_token: String::new(),
            default_agent: "default".into(),
            default_peer: None,
            output_format: "table".into(),
            poll_interval_ms: 500,
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            keepalive_ms: 25000,
            nat_override: None,
            rendezvous_server: "udp://ashir.world:40004".into(),
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
            let cfg = Self::default();
            let _ = cfg.save();
            cfg
        };
        cfg.apply_env_overrides();
        Ok(cfg)
    }

    pub fn save(&self) -> Result<(), crate::errors::CliError> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(self).map_err(|e| crate::errors::CliError::Parse {
            message: e.to_string(),
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
        if let Ok(v) = std::env::var("ADTP_DEFAULT_PEER") {
            self.default_peer = Some(v);
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
        if let Ok(v) = std::env::var("ADTP_KEEPALIVE") {
            if let Ok(n) = v.parse() {
                self.keepalive_ms = n;
            }
        }
        if let Ok(v) = std::env::var("ADTP_RENDEZVOUS") {
            self.rendezvous_server = v;
        }
        if let Ok(v) = std::env::var("ADTP_NAT_OVERRIDE") {
            self.nat_override = Some(v);
        }
    }

    pub fn validate(&self) -> Result<(), crate::errors::CliError> {
        if self.api_token.is_empty() {
            return Err(crate::errors::CliError::Config {
                message: "ADTP_API_TOKEN is not set. Set it in cli.toml or ADTP_API_TOKEN env var."
                    .into(),
            });
        }
        Ok(())
    }

    pub fn token_from_keychain(&self) -> Option<String> {
        let service = "adtp-cli";
        let entry = keyring::Entry::new(service, "api_token").ok()?;
        entry.get_password().ok()
    }

    pub fn save_token_to_keychain(&self, token: &str) -> Result<(), crate::errors::CliError> {
        let service = "adtp-cli";
        let entry =
            keyring::Entry::new(service, "api_token").map_err(|e| crate::errors::CliError::Io {
                message: format!("Keychain error: {e}"),
            })?;
        entry
            .set_password(token)
            .map_err(|e| crate::errors::CliError::Io {
                message: format!("Failed to save token: {e}"),
            })?;
        Ok(())
    }
}
