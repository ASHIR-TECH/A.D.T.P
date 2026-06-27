use std::sync::Arc;
use tokio::sync::Mutex;
use crate::agent::config::AgentConfig;
use crate::agent::permissions::{PermissionConfig, PermissionStore};
use crate::agent::provider::{LlmMessage, Provider, ToolDefinition};
use crate::agent::anthropic::AnthropicProvider;
use crate::agent::openai::OpenAiProvider;
use crate::agent::ollama::OllamaProvider;

#[derive(Debug, Clone)]
pub enum AgentState {
    Inactive,
    Active,
    Error(String),
}

pub struct AgentRuntime {
    config: AgentConfig,
    permission_store: PermissionStore,
    state: Arc<Mutex<AgentState>>,
    provider: Arc<Mutex<Option<Box<dyn Provider>>>>,
    conversation: Arc<Mutex<Vec<ConversationEntry>>>,
    tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationEntry {
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    pub tool_calls: Vec<String>,
}

impl AgentRuntime {
    pub fn new(config: AgentConfig, perm_config: PermissionConfig) -> Self {
        let tools = vec![
            ToolDefinition {
                name: "send_file".into(),
                description: "Send a local file to another peer".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "agent_name": {"type": "string"},
                        "priority": {"type": "string", "enum": ["low", "normal", "high"]},
                    },
                    "required": ["file_path"],
                }),
            },
            ToolDefinition {
                name: "list_transfers".into(),
                description: "List recent file transfers".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "status": {"type": "string"},
                        "limit": {"type": "integer"},
                    },
                }),
            },
            ToolDefinition {
                name: "find_file".into(),
                description: "Search the local filesystem for a file by name or pattern".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string", "description": "Filename or glob pattern"},
                        "directory": {"type": "string", "description": "Directory to search in"},
                        "extension": {"type": "string", "description": "File extension filter"},
                    },
                    "required": ["pattern"],
                }),
            },
            ToolDefinition {
                name: "read_file".into(),
                description: "Read the contents of a local file".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Absolute path to the file"},
                    },
                    "required": ["path"],
                }),
            },
            ToolDefinition {
                name: "list_directory".into(),
                description: "List contents of a directory".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Absolute path to the directory"},
                    },
                    "required": ["path"],
                }),
            },
            ToolDefinition {
                name: "shell_exec".into(),
                description: "Execute a permitted shell command".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "Command to execute"},
                    },
                    "required": ["command"],
                }),
            },
        ];

        Self {
            permission_store: PermissionStore::new(perm_config),
            config,
            state: Arc::new(Mutex::new(AgentState::Inactive)),
            provider: Arc::new(Mutex::new(None)),
            conversation: Arc::new(Mutex::new(Vec::new())),
            tools,
        }
    }

    pub async fn activate(&self) -> Result<(), String> {
        let provider: Box<dyn Provider> = match self.config.provider.as_str() {
            "anthropic" => {
                let key = self.config.api_key.as_ref().ok_or("Anthropic requires an API key")?;
                Box::new(AnthropicProvider::new(
                    key.clone(), self.config.model.clone(), self.config.max_tokens,
                ))
            }
            "openai" => {
                let key = self.config.api_key.as_ref().ok_or("OpenAI requires an API key")?;
                Box::new(OpenAiProvider::new(
                    key.clone(), self.config.model.clone(), self.config.max_tokens,
                ))
            }
            "ollama" => {
                let host = self.config.ollama_host.clone().unwrap_or_else(|| "http://localhost:11434".into());
                let provider = OllamaProvider::new(host, self.config.model.clone(), self.config.max_tokens);
                let available = provider.check_available().await?;
                if !available {
                    return Err(format!("Model '{}' not found in Ollama. Pull it first with: ollama pull {}", self.config.model, self.config.model));
                }
                Box::new(provider)
            }
            other => return Err(format!("Unknown provider: {}", other)),
        };

        let mut p = self.provider.lock().await;
        *p = Some(provider);
        let mut state = self.state.lock().await;
        *state = AgentState::Active;
        Ok(())
    }

    pub async fn deactivate(&self) {
        let mut p = self.provider.lock().await;
        *p = None;
        let mut state = self.state.lock().await;
        *state = AgentState::Inactive;
    }

    pub async fn state(&self) -> AgentState {
        self.state.lock().await.clone()
    }

    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    pub fn permission_store(&self) -> &PermissionStore {
        &self.permission_store
    }

    pub fn tools(&self) -> &[ToolDefinition] {
        &self.tools
    }

    pub async fn send_message(&self, content: &str) -> Result<String, String> {
        let provider_guard = self.provider.lock().await;
        let provider = provider_guard.as_ref().ok_or("Agent is not activated")?;

        let system_prompt = self.build_system_prompt();

        let mut history = self.conversation.lock().await;
        history.push(ConversationEntry {
            role: "user".into(),
            content: content.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            tool_calls: vec![],
        });

        let messages: Vec<LlmMessage> = history.iter().map(|e| LlmMessage {
            role: e.role.clone(),
            content: e.content.clone(),
        }).collect();

        let response = provider.send_message(&messages, &system_prompt, &self.tools).await?;

        history.push(ConversationEntry {
            role: "assistant".into(),
            content: response.content.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            tool_calls: response.tool_calls.iter().map(|t| t.name.clone()).collect(),
        });

        Ok(response.content)
    }

    pub async fn conversation(&self) -> Vec<ConversationEntry> {
        self.conversation.lock().await.clone()
    }

    pub async fn clear_conversation(&self) {
        self.conversation.lock().await.clear();
    }

    fn build_system_prompt(&self) -> String {
        let allowed = self.permission_store.config().allowed_paths.join(", ");
        let writable = self.permission_store.config().writable_paths.join(", ");
        let commands = self.permission_store.config().allowed_commands.join(", ");
        let allowed_str = if allowed.is_empty() { "none (you cannot read any files yet)" } else { &allowed };
        let writable_str = if writable.is_empty() { "none (you cannot write files yet)" } else { &writable };
        let commands_str = if commands.is_empty() { "none (you cannot run shell commands)" } else { &commands };

        let mut prompt = format!(
            r#"You are the ADTP agent running locally. You manage files and tasks on this machine.

You have access to the following tools: send_file, list_transfers, find_file, read_file, list_directory, shell_exec.

You are allowed to read files from: {allowed_str}
You are allowed to write to: {writable_str}
You may execute: {commands_str}
You must not access any path or run any command not listed here.

Always explain what you are doing before using a tool.
When you receive a file, acknowledge it and describe what you can do with it.
If a permission is denied, tell the user clearly which permission needs to be granted."#
        );

        if let Some(override_text) = &self.config.system_prompt_override {
            prompt.push_str("\n\nAdditional instructions from user:\n");
            prompt.push_str(override_text);
        }

        prompt
    }
}
