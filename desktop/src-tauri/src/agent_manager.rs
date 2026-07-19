use adtp_peer_core::agent::multi_config::{AgentEntry, AgentsConfig};
use adtp_peer_core::agent::permissions::PermissionConfig;
use adtp_peer_core::agent::{AgentRuntime, ToolExecutor};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AddAgentRequest {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub ollama_host: Option<String>,
    pub max_tokens: u32,
    pub permissions: PermissionConfig,
    pub enabled: bool,
}

pub struct AgentManager {
    runtimes: Arc<Mutex<HashMap<String, AgentRuntime>>>,
    entries: Arc<Mutex<Vec<AgentEntry>>>,
}

impl AgentManager {
    pub fn new() -> Self {
        let config = AgentsConfig::migrate_from_old().unwrap_or_default();
        let entries = config.agents.clone();

        let mut runtimes = HashMap::new();
        for entry in &entries {
            if entry.enabled {
                let perm_config =
                    adtp_peer_core::agent::permissions::load_permissions().unwrap_or_default();
                let agent_config = entry.to_agent_config();
                let runtime = AgentRuntime::new(agent_config, perm_config);
                runtimes.insert(entry.id.clone(), runtime);
            }
        }

        Self {
            runtimes: Arc::new(Mutex::new(runtimes)),
            entries: Arc::new(Mutex::new(entries)),
        }
    }

    pub async fn set_tool_executor(&self, executor: Arc<dyn ToolExecutor>) {
        let mut runtimes = self.runtimes.lock().await;
        for runtime in runtimes.values_mut() {
            runtime.set_tool_executor(executor.clone());
        }
    }

    pub async fn list_agents(&self) -> Vec<AgentEntry> {
        self.entries.lock().await.clone()
    }

    pub async fn add_agent(&self, request: AddAgentRequest) -> Result<AgentEntry, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let name = format!("{} ({})", &request.provider, &request.model);

        let entry = AgentEntry {
            id: id.clone(),
            name,
            provider: request.provider,
            model: request.model,
            api_key: if request.api_key.is_empty() {
                None
            } else {
                Some(request.api_key)
            },
            ollama_host: request.ollama_host,
            max_tokens: request.max_tokens,
            temperature: 0.7,
            enabled: request.enabled,
        };

        adtp_peer_core::agent::permissions::save_permissions(&request.permissions)?;

        if request.enabled {
            let agent_config = entry.to_agent_config();
            let runtime = AgentRuntime::new(agent_config, request.permissions);
            runtime.activate().await?;
            let mut runtimes = self.runtimes.lock().await;
            runtimes.insert(id.clone(), runtime);
        }

        let mut entries = self.entries.lock().await;
        entries.push(entry.clone());
        let config = AgentsConfig {
            agents: entries.clone(),
        };
        config.save()?;

        Ok(entry)
    }

    pub async fn remove_agent(&self, id: String) -> Result<(), String> {
        {
            let mut runtimes = self.runtimes.lock().await;
            if let Some(runtime) = runtimes.remove(&id) {
                runtime.deactivate().await;
            }
        }
        let mut entries = self.entries.lock().await;
        entries.retain(|e| e.id != id);
        let config = AgentsConfig {
            agents: entries.clone(),
        };
        config.save()?;
        Ok(())
    }

    pub async fn toggle_agent(&self, id: String, enabled: bool) -> Result<(), String> {
        let entry;
        {
            let mut entries = self.entries.lock().await;
            let e = entries
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or_else(|| "Agent not found".to_string())?;
            e.enabled = enabled;
            entry = e.clone();
        }

        if enabled {
            let mut runtimes = self.runtimes.lock().await;
            if !runtimes.contains_key(&id) {
                let perm_config =
                    adtp_peer_core::agent::permissions::load_permissions().unwrap_or_default();
                let agent_config = entry.to_agent_config();
                let runtime = AgentRuntime::new(agent_config, perm_config);
                runtime.activate().await?;
                runtimes.insert(id.clone(), runtime);
            }
        } else {
            let mut runtimes = self.runtimes.lock().await;
            if let Some(runtime) = runtimes.remove(&id) {
                runtime.deactivate().await;
            }
        }

        let entries = self.entries.lock().await;
        let config = AgentsConfig {
            agents: entries.clone(),
        };
        config.save()?;
        Ok(())
    }

    pub async fn send_message(
        &self,
        agent_id: Option<String>,
        message: String,
    ) -> Result<String, String> {
        let runtimes = self.runtimes.lock().await;
        match agent_id {
            Some(id) => {
                let runtime = runtimes
                    .get(&id)
                    .ok_or_else(|| "Agent not found or not enabled".to_string())?;
                runtime.send_message(&message).await
            }
            None => {
                let runtime = runtimes
                    .values()
                    .next()
                    .ok_or_else(|| "No active agents".to_string())?;
                runtime.send_message(&message).await
            }
        }
    }

    pub async fn get_conversation(
        &self,
        agent_id: Option<String>,
    ) -> Vec<adtp_peer_core::agent::runtime::ConversationEntry> {
        let runtimes = self.runtimes.lock().await;
        let runtime = match agent_id {
            Some(id) => runtimes.get(&id),
            None => runtimes.values().next(),
        };
        match runtime {
            Some(r) => r.conversation().await,
            None => vec![],
        }
    }

    pub async fn clear_conversation(&self, agent_id: Option<String>) {
        let runtimes = self.runtimes.lock().await;
        match agent_id {
            Some(id) => {
                if let Some(r) = runtimes.get(&id) {
                    r.clear_conversation().await;
                }
            }
            None => {
                if let Some(r) = runtimes.values().next() {
                    r.clear_conversation().await;
                }
            }
        }
    }
}
