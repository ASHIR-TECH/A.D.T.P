use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub ollama_host: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub system_prompt_override: Option<String>,
    pub can_task_from: Vec<String>,
    pub queue_expiry_hours: u64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            api_key: None,
            ollama_host: None,
            max_tokens: 4096,
            temperature: 0.7,
            system_prompt_override: None,
            can_task_from: vec!["*".into()],
            queue_expiry_hours: 24,
        }
    }
}

pub fn config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("adtp").join("agent.toml"))
}

pub fn load_config() -> Result<Option<AgentConfig>, String> {
    let path = match config_path() {
        Some(p) => p,
        None => return Err("Cannot determine config directory".into()),
    };
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    toml::from_str(&content).map(Some).map_err(|e| e.to_string())
}

pub fn save_config(config: &AgentConfig) -> Result<(), String> {
    let path = config_path().ok_or("Cannot determine config directory")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, &content).map_err(|e| e.to_string())
}

pub fn delete_config() -> Result<(), String> {
    let path = config_path().ok_or("Cannot determine config directory")?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AgentConfig::default();
        assert_eq!(config.provider, "openai");
        assert_eq!(config.model, "gpt-4o");
        assert!(config.api_key.is_none());
        assert_eq!(config.max_tokens, 4096);
        assert_eq!(config.queue_expiry_hours, 24);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = AgentConfig {
            provider: "anthropic".into(),
            model: "claude-sonnet-4-20250514".into(),
            api_key: Some("sk-test-key".into()),
            max_tokens: 8192,
            temperature: 0.5,
            system_prompt_override: Some("Be concise.".into()),
            can_task_from: vec!["peer-1".into()],
            queue_expiry_hours: 48,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.provider, "anthropic");
        assert_eq!(loaded.model, "claude-sonnet-4-20250514");
        assert_eq!(loaded.api_key, Some("sk-test-key".into()));
        assert_eq!(loaded.max_tokens, 8192);
        assert_eq!(loaded.temperature, 0.5);
        assert_eq!(loaded.system_prompt_override, Some("Be concise.".into()));
        assert_eq!(loaded.can_task_from, vec!["peer-1"]);
        assert_eq!(loaded.queue_expiry_hours, 48);
    }

    #[test]
    fn test_config_path_is_in_config_dir() {
        let path = config_path();
        assert!(path.is_some());
        let path_str = path.unwrap().to_string_lossy().to_string();
        assert!(path_str.contains("adtp"));
        assert!(path_str.contains("agent.toml"));
    }
}
