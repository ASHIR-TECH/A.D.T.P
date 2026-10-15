use super::config::AgentConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub ollama_host: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub enabled: bool,
}

impl AgentEntry {
    pub fn to_agent_config(&self) -> AgentConfig {
        AgentConfig {
            provider: self.provider.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            ollama_host: self.ollama_host.clone(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentsConfig {
    pub agents: Vec<AgentEntry>,
    /// Set once the user configures a provider of their own. The shared Base AI
    /// is then retired for good — matching the "once you add your keys you
    /// can't use mine again" rule.
    #[serde(default)]
    pub base_ai_retired: bool,
}

impl AgentsConfig {
    pub fn path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|p| p.join("adtp").join("agents.toml"))
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::path().ok_or("Cannot determine config directory")?;
        if !path.exists() {
            return Ok(Self {
                agents: vec![],
                ..Default::default()
            });
        }
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("Cannot determine config directory")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, &content).map_err(|e| e.to_string())
    }

    pub fn migrate_from_old() -> Result<Self, String> {
        let old_path = dirs::config_dir()
            .map(|p| p.join("adtp").join("agent.toml"))
            .ok_or("Cannot determine config directory")?;
        let new_path = Self::path().ok_or("Cannot determine config directory")?;

        if new_path.exists() {
            return Self::load();
        }

        if !old_path.exists() {
            return Ok(Self {
                agents: vec![],
                ..Default::default()
            });
        }

        let content = std::fs::read_to_string(&old_path).map_err(|e| e.to_string())?;
        let old: AgentConfig = toml::from_str(&content).map_err(|e| e.to_string())?;

        let entry = AgentEntry {
            id: uuid::Uuid::new_v4().to_string(),
            name: format!("{} ({})", old.provider, old.model),
            provider: old.provider,
            model: old.model,
            api_key: old.api_key,
            ollama_host: old.ollama_host,
            max_tokens: old.max_tokens,
            temperature: old.temperature,
            enabled: true,
        };

        let config = Self {
            agents: vec![entry],
            ..Default::default()
        };
        config.save()?;
        let _ = std::fs::remove_file(&old_path);
        Ok(config)
    }
}
