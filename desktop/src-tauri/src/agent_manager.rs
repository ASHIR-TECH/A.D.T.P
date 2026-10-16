use adtp_peer_core::agent::base::{BASE_AGENT_ID, BASE_PROVIDER};
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
    retired: Arc<Mutex<bool>>,
}

/// How the built-in shared assistant should be reconciled with the stored
/// agents on startup.
#[derive(Debug, PartialEq, Eq)]
pub enum BaseAiPlan {
    /// The user has no keys of their own — make sure the shared agent exists.
    Ensure,
    /// The user brought their own key — retire the shared agent permanently.
    Retire,
}

/// Pure decision function so the BYOK rule is testable without touching disk.
pub fn plan_base_ai(entries: &[AgentEntry], retired: bool) -> BaseAiPlan {
    if retired || entries.iter().any(|e| e.provider != BASE_PROVIDER) {
        BaseAiPlan::Retire
    } else {
        BaseAiPlan::Ensure
    }
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
            retired: Arc::new(Mutex::new(config.base_ai_retired)),
        }
    }

    /// Whether the shared Base AI has been permanently retired because the user
    /// configured their own provider keys.
    pub async fn base_ai_retired(&self) -> bool {
        *self.retired.lock().await
    }

    async fn persist(&self) -> Result<(), String> {
        let agents = self.entries.lock().await.clone();
        let base_ai_retired = *self.retired.lock().await;
        AgentsConfig {
            agents,
            base_ai_retired,
        }
        .save()
    }

    /// Reconciles the built-in shared assistant with the stored agents. Called
    /// at startup: installs the Base AI for users who have no keys of their
    /// own, and retires it permanently for users who do.
    pub async fn ensure_base_ai(&self) -> Result<(), String> {
        let retired = *self.retired.lock().await;
        let entries = self.entries.lock().await.clone();

        match plan_base_ai(&entries, retired) {
            BaseAiPlan::Retire => self.retire_base_ai().await,
            BaseAiPlan::Ensure => self.install_base_ai().await,
        }
    }

    /// Installs (or re-enables) the shared agent and activates its runtime.
    async fn install_base_ai(&self) -> Result<(), String> {
        let entry = AgentEntry {
            id: BASE_AGENT_ID.to_string(),
            name: "Base AI (shared)".to_string(),
            provider: BASE_PROVIDER.to_string(),
            model: "mars-base".to_string(),
            api_key: None,
            ollama_host: None,
            max_tokens: 2048,
            temperature: 0.7,
            enabled: true,
        };

        {
            let mut runtimes = self.runtimes.lock().await;
            if let Some(runtime) = runtimes.get(BASE_AGENT_ID) {
                // A restart recreates runtimes inactive; make sure the shared
                // agent is live even without the separate activate sweep.
                if !matches!(
                    runtime.state().await,
                    adtp_peer_core::agent::runtime::AgentState::Active
                ) {
                    runtime.activate().await?;
                }
            } else {
                let perm_config =
                    adtp_peer_core::agent::permissions::load_permissions().unwrap_or_default();
                let runtime = AgentRuntime::new(entry.to_agent_config(), perm_config);
                runtime.activate().await?;
                runtimes.insert(BASE_AGENT_ID.to_string(), runtime);
            }
        }

        let changed = {
            let mut entries = self.entries.lock().await;
            match entries.iter_mut().find(|e| e.id == BASE_AGENT_ID) {
                Some(existing) => {
                    let was_enabled = existing.enabled;
                    existing.enabled = true;
                    !was_enabled
                }
                None => {
                    entries.push(entry);
                    true
                }
            }
        };

        if changed {
            self.persist().await?;
        }
        Ok(())
    }

    /// Removes the shared agent and remembers that it must stay gone.
    async fn retire_base_ai(&self) -> Result<(), String> {
        let mut changed = false;

        {
            let mut runtimes = self.runtimes.lock().await;
            if let Some(runtime) = runtimes.remove(BASE_AGENT_ID) {
                runtime.deactivate().await;
                changed = true;
            }
        }

        {
            let mut entries = self.entries.lock().await;
            let before = entries.len();
            entries.retain(|e| e.id != BASE_AGENT_ID);
            if entries.len() != before {
                changed = true;
            }
        }

        {
            let mut retired = self.retired.lock().await;
            if !*retired {
                *retired = true;
                changed = true;
            }
        }

        if changed {
            self.persist().await?;
        }
        Ok(())
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

    /// Coarse status for the mobile companion: whether any runtime is actually
    /// activated, plus the provider/model of the first enabled agent.
    pub async fn status(&self) -> (bool, Option<String>, Option<String>) {
        let runtimes = self.runtimes.lock().await;
        let mut active = false;
        for runtime in runtimes.values() {
            if matches!(
                runtime.state().await,
                adtp_peer_core::agent::runtime::AgentState::Active
            ) {
                active = true;
                break;
            }
        }
        drop(runtimes);

        let entries = self.entries.lock().await;
        let entry = entries
            .iter()
            .find(|e| e.enabled)
            .or_else(|| entries.first());
        (
            active,
            entry.map(|e| e.provider.clone()),
            entry.map(|e| e.model.clone()),
        )
    }

    /// Activates every enabled agent restored from disk. `new()` builds the
    /// runtimes but leaves them inactive, so without this a freshly started
    /// desktop answers "Agent is not activated" to every message.
    pub async fn activate_enabled(&self) {
        let runtimes = self.runtimes.lock().await;
        for (id, runtime) in runtimes.iter() {
            if let Err(error) = runtime.activate().await {
                tracing::warn!("agent {id} could not be activated: {error}");
            }
        }
    }

    pub async fn add_agent(&self, request: AddAgentRequest) -> Result<AgentEntry, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let name = format!("{} ({})", &request.provider, &request.model);
        let is_byo = request.provider != BASE_PROVIDER;

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

        {
            let mut entries = self.entries.lock().await;
            entries.push(entry.clone());
        }
        self.persist().await?;

        // Bring-your-own-key: adding your own provider retires the shared Base
        // AI permanently.
        if is_byo {
            self.retire_base_ai().await?;
        }

        Ok(entry)
    }

    pub async fn remove_agent(&self, id: String) -> Result<(), String> {
        {
            let mut runtimes = self.runtimes.lock().await;
            if let Some(runtime) = runtimes.remove(&id) {
                runtime.deactivate().await;
            }
        }
        {
            let mut entries = self.entries.lock().await;
            entries.retain(|e| e.id != id);
        }
        self.persist().await
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

        self.persist().await
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
                // Deterministic selection (config order, first enabled) so the
                // agent that answers matches the provider/model `status()`
                // reports — never HashMap iteration order.
                let entries = self.entries.lock().await;
                for entry in entries.iter().filter(|e| e.enabled) {
                    if let Some(runtime) = runtimes.get(&entry.id) {
                        return runtime.send_message(&message).await;
                    }
                }
                Err("No active agents".to_string())
            }
        }
    }

    /// The agent used when a caller doesn't specify one: first enabled entry
    /// (falling back to the first entry), matching `send_message`.
    async fn default_agent_id(&self) -> Option<String> {
        let entries = self.entries.lock().await;
        entries
            .iter()
            .find(|e| e.enabled)
            .or_else(|| entries.first())
            .map(|e| e.id.clone())
    }

    pub async fn get_conversation(
        &self,
        agent_id: Option<String>,
    ) -> Vec<adtp_peer_core::agent::runtime::ConversationEntry> {
        let id = match agent_id {
            Some(id) => Some(id),
            None => self.default_agent_id().await,
        };
        let runtimes = self.runtimes.lock().await;
        match id.and_then(|id| runtimes.get(&id)) {
            Some(r) => r.conversation().await,
            None => vec![],
        }
    }

    pub async fn clear_conversation(&self, agent_id: Option<String>) {
        let id = match agent_id {
            Some(id) => Some(id),
            None => self.default_agent_id().await,
        };
        let runtimes = self.runtimes.lock().await;
        if let Some(r) = id.and_then(|id| runtimes.get(&id)) {
            r.clear_conversation().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// `AgentsConfig` and the permission store both resolve under
    /// `dirs::config_dir()`, so pointing `XDG_CONFIG_HOME` at a scratch dir
    /// isolates these tests from the developer's real ADTP config.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_config<R>(f: impl FnOnce() -> R) -> R {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var_os("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().expect("temp config dir");
        std::env::set_var("XDG_CONFIG_HOME", dir.path());

        let out = f();

        match previous {
            Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        out
    }

    fn entry(provider: &str) -> AgentEntry {
        AgentEntry {
            id: format!("{provider}-1"),
            name: format!("{provider} agent"),
            provider: provider.to_string(),
            model: "some-model".to_string(),
            api_key: Some("key".to_string()),
            ollama_host: None,
            max_tokens: 1024,
            temperature: 0.7,
            enabled: true,
        }
    }

    #[test]
    fn plan_ensures_shared_ai_when_no_user_keys() {
        assert_eq!(plan_base_ai(&[], false), BaseAiPlan::Ensure);
        assert_eq!(
            plan_base_ai(&[entry(BASE_PROVIDER)], false),
            BaseAiPlan::Ensure
        );
    }

    #[test]
    fn plan_retires_once_user_adds_own_provider() {
        assert_eq!(
            plan_base_ai(&[entry("anthropic")], false),
            BaseAiPlan::Retire
        );
        assert_eq!(
            plan_base_ai(&[entry(BASE_PROVIDER), entry("openai")], false),
            BaseAiPlan::Retire
        );
        // Ollama is local but still the user's own choice — same rule.
        assert_eq!(plan_base_ai(&[entry("ollama")], false), BaseAiPlan::Retire);
    }

    #[test]
    fn plan_never_reinstalls_after_retirement() {
        assert_eq!(plan_base_ai(&[], true), BaseAiPlan::Retire);
        assert_eq!(
            plan_base_ai(&[entry(BASE_PROVIDER)], true),
            BaseAiPlan::Retire
        );
    }

    #[test]
    fn ensure_base_ai_installs_and_activates_shared_agent() {
        with_temp_config(|| {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let mgr = AgentManager::new();
                mgr.ensure_base_ai().await.unwrap();

                let agents = mgr.list_agents().await;
                let base = agents
                    .iter()
                    .find(|a| a.provider == BASE_PROVIDER)
                    .expect("shared agent installed");
                assert!(base.enabled);

                let (active, provider, _model) = mgr.status().await;
                assert!(active, "shared agent runtime should be active");
                assert_eq!(provider.as_deref(), Some(BASE_PROVIDER));

                // Idempotent: a second call must not duplicate the entry.
                mgr.ensure_base_ai().await.unwrap();
                let count = mgr
                    .list_agents()
                    .await
                    .iter()
                    .filter(|a| a.provider == BASE_PROVIDER)
                    .count();
                assert_eq!(count, 1);

                // A restart (fresh manager, runtime built inactive) must come
                // back up active after reconciliation alone.
                let restarted = AgentManager::new();
                restarted.ensure_base_ai().await.unwrap();
                let (active, provider, _) = restarted.status().await;
                assert!(active, "shared agent should be active after restart");
                assert_eq!(provider.as_deref(), Some(BASE_PROVIDER));
            });
        });
    }

    #[test]
    fn adding_own_agent_retires_shared_ai_for_good() {
        with_temp_config(|| {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let mgr = AgentManager::new();
                mgr.ensure_base_ai().await.unwrap();
                assert!(mgr
                    .list_agents()
                    .await
                    .iter()
                    .any(|a| a.provider == BASE_PROVIDER));

                mgr.add_agent(AddAgentRequest {
                    provider: "ollama".to_string(),
                    model: "llama3".to_string(),
                    api_key: String::new(),
                    ollama_host: Some("http://localhost:11434".to_string()),
                    max_tokens: 1024,
                    permissions: PermissionConfig::default(),
                    enabled: false,
                })
                .await
                .unwrap();

                assert!(mgr.base_ai_retired().await);
                assert!(!mgr
                    .list_agents()
                    .await
                    .iter()
                    .any(|a| a.provider == BASE_PROVIDER));

                // A restart must not resurrect the shared assistant.
                let restarted = AgentManager::new();
                restarted.ensure_base_ai().await.unwrap();
                assert!(!restarted
                    .list_agents()
                    .await
                    .iter()
                    .any(|a| a.provider == BASE_PROVIDER));
            });
        });
    }
}
