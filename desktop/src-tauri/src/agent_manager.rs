use adtp_peer_core::agent::{
    AgentConfig, AgentRuntime,
};
use adtp_peer_core::agent::permissions::PermissionConfig;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AgentManager {
    runtime: Mutex<AgentRuntime>,
    active: Arc<Mutex<bool>>,
    last_active: Arc<Mutex<Option<i64>>>,
}

impl AgentManager {
    pub fn new() -> Self {
        let perm_config = adtp_peer_core::agent::permissions::load_permissions()
            .unwrap_or_default();
        let agent_config = adtp_peer_core::agent::config::load_config()
            .unwrap_or_else(|_| None)
            .unwrap_or_default();

        Self {
            runtime: Mutex::new(AgentRuntime::new(agent_config, perm_config)),
            active: Arc::new(Mutex::new(false)),
            last_active: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn activate(
        &self,
        provider: String,
        model: String,
        api_key: String,
        ollama_host: Option<String>,
        max_tokens: u32,
        permissions: PermissionConfig,
    ) -> Result<(), String> {
        let mut config = AgentConfig::default();
        config.provider = provider;
        config.model = model;
        if !api_key.is_empty() {
            config.api_key = Some(api_key.clone());
        }
        config.ollama_host = ollama_host;
        config.max_tokens = max_tokens;

        adtp_peer_core::agent::permissions::save_permissions(&permissions)?;
        adtp_peer_core::agent::config::save_config(&config)?;

        self.runtime.lock().await.deactivate().await;
        let new_runtime = AgentRuntime::new(config, permissions);
        new_runtime.activate().await?;
        *self.runtime.lock().await = new_runtime;

        let mut active = self.active.lock().await;
        *active = true;
        let mut last = self.last_active.lock().await;
        *last = Some(chrono::Utc::now().timestamp());

        Ok(())
    }

    pub async fn deactivate(&self) -> Result<(), String> {
        self.runtime.lock().await.deactivate().await;
        let mut active = self.active.lock().await;
        *active = false;
        Ok(())
    }

    pub async fn status(&self) -> serde_json::Value {
        let active = *self.active.lock().await;
        let last = *self.last_active.lock().await;
        let config = self.runtime.lock().await.config().clone();
        serde_json::json!({
            "active": active,
            "provider": config.provider,
            "model": config.model,
            "ollama_host": config.ollama_host,
            "max_tokens": config.max_tokens,
            "last_active": last,
        })
    }

    pub async fn send_message(&self, message: String) -> Result<String, String> {
        if !*self.active.lock().await {
            return Err("Agent is not active".into());
        }
        self.runtime.lock().await.send_message(&message).await
    }

    pub async fn permissions(&self) -> PermissionConfig {
        self.runtime.lock().await.permission_store().config().clone()
    }

    pub async fn update_permissions(&self, config: PermissionConfig) -> Result<(), String> {
        adtp_peer_core::agent::permissions::save_permissions(&config)?;
        Ok(())
    }

    pub async fn conversation(&self) -> Vec<adtp_peer_core::agent::runtime::ConversationEntry> {
        self.runtime.lock().await.conversation().await
    }

    pub async fn clear_conversation(&self) {
        self.runtime.lock().await.clear_conversation().await
    }
}
