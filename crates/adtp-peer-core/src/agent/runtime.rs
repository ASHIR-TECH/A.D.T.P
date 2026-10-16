use crate::agent::anthropic::AnthropicProvider;
use crate::agent::base::BaseProvider;
use crate::agent::config::AgentConfig;
use crate::agent::ollama::OllamaProvider;
use crate::agent::openai::OpenAiProvider;
use crate::agent::permissions::{PermissionConfig, PermissionStore};
use crate::agent::provider::{LlmMessage, LlmResponse, Provider, ToolCall, ToolDefinition};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub enum AgentState {
    Inactive,
    Active,
    Error(String),
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn send_file(&self, file_path: &str, peer_id: &str) -> Result<String, String>;
    async fn list_peers(&self) -> Result<String, String>;
    async fn get_own_peer_id(&self) -> Result<String, String>;
    async fn list_transfers(&self, status: Option<&str>) -> Result<String, String>;
}

pub struct AgentRuntime {
    config: AgentConfig,
    permission_store: PermissionStore,
    state: Arc<Mutex<AgentState>>,
    provider: Arc<Mutex<Option<Box<dyn Provider>>>>,
    conversation: Arc<Mutex<Vec<ConversationEntry>>>,
    tools: Vec<ToolDefinition>,
    executor: Option<Arc<dyn ToolExecutor>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationEntry {
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    pub tool_calls: Vec<String>,
    /// Full detail of every tool invocation the assistant made in this turn
    /// (id, name, status, arguments, result). Names-only `tool_calls` is kept
    /// for the desktop UI; this lets the mobile companion render file tiles
    /// (send_file params) and per-step results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_details: Vec<ToolCallDetail>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCallDetail {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub result: Option<String>,
}

impl AgentRuntime {
    pub fn new(config: AgentConfig, perm_config: PermissionConfig) -> Self {
        let tools = vec![
            ToolDefinition {
                name: "send_file".into(),
                description: "Send a local file to another peer by peer ID. Use this when the user asks to send a file to another computer.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string", "description": "Absolute path to the file on this machine"},
                        "peer_id": {"type": "string", "description": "Peer ID of the destination computer"},
                    },
                    "required": ["file_path", "peer_id"],
                }),
            },
            ToolDefinition {
                name: "list_peers".into(),
                description: "List known remote peers and their online status".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                }),
            },
            ToolDefinition {
                name: "list_transfers".into(),
                description: "List recent file transfers. Optionally filter by status (completed, failed, in_progress)".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "status": {"type": "string", "description": "Filter by status: completed, failed, or in_progress"},
                    },
                }),
            },
            ToolDefinition {
                name: "get_own_peer_id".into(),
                description: "Get this computer's own peer ID. Use this when the user asks for their peer ID.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
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
            executor: None,
        }
    }

    pub fn set_tool_executor(&mut self, executor: Arc<dyn ToolExecutor>) {
        self.executor = Some(executor);
    }

    pub async fn activate(&self) -> Result<(), String> {
        let provider: Box<dyn Provider> = match self.config.provider.as_str() {
            "anthropic" => {
                let key = self
                    .config
                    .api_key
                    .as_ref()
                    .ok_or("Anthropic requires an API key")?;
                Box::new(AnthropicProvider::new(
                    key.clone(),
                    self.config.model.clone(),
                    self.config.max_tokens,
                ))
            }
            "openai" => {
                let key = self
                    .config
                    .api_key
                    .as_ref()
                    .ok_or("OpenAI requires an API key")?;
                Box::new(OpenAiProvider::new(
                    key.clone(),
                    self.config.model.clone(),
                    self.config.max_tokens,
                ))
            }
            "ollama" => {
                let host = self
                    .config
                    .ollama_host
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".into());
                let provider =
                    OllamaProvider::new(host, self.config.model.clone(), self.config.max_tokens);
                let available = provider.check_available().await?;
                if !available {
                    return Err(format!(
                        "Model '{}' not found in Ollama. Pull it first with: ollama pull {}",
                        self.config.model, self.config.model
                    ));
                }
                Box::new(provider)
            }
            "base" => Box::new(BaseProvider::new()),
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

    /// Test-only: install a provider without going through `activate()`, so the
    /// tool loop can be exercised without live API credentials.
    #[cfg(test)]
    pub async fn set_provider_for_test(&self, provider: Box<dyn Provider>) {
        *self.provider.lock().await = Some(provider);
        *self.state.lock().await = AgentState::Active;
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
        let _provider = provider_guard.as_ref().ok_or("Agent is not activated")?;
        let system_prompt = self.build_system_prompt();

        let mut history = self.conversation.lock().await;
        history.push(ConversationEntry {
            role: "user".into(),
            content: content.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            tool_calls: vec![],
            tool_details: vec![],
        });

        let executor = self.executor.clone();
        drop(provider_guard);
        drop(history);

        let max_iterations = 10;
        for _iteration in 0..max_iterations {
            let hist = self.conversation.lock().await;
            let messages: Vec<LlmMessage> = hist
                .iter()
                .map(|e| LlmMessage {
                    role: e.role.clone(),
                    content: e.content.clone(),
                })
                .collect();
            drop(hist);

            let provider_guard = self.provider.lock().await;
            let response: LlmResponse = provider_guard
                .as_ref()
                .ok_or("Agent is not activated")?
                .send_message(&messages, &system_prompt, &self.tools)
                .await?;
            drop(provider_guard);

            let has_tool_calls = !response.tool_calls.is_empty();

            let mut hist = self.conversation.lock().await;
            let tool_details: Vec<ToolCallDetail> = response
                .tool_calls
                .iter()
                .map(|t| ToolCallDetail {
                    id: t.id.clone(),
                    name: t.name.clone(),
                    status: "running".into(),
                    params: t.arguments.clone(),
                    result: None,
                })
                .collect();
            hist.push(ConversationEntry {
                role: "assistant".into(),
                content: response.content.clone(),
                timestamp: chrono::Utc::now().timestamp(),
                tool_calls: response.tool_calls.iter().map(|t| t.name.clone()).collect(),
                tool_details,
            });

            if !has_tool_calls {
                return Ok(response.content);
            }

            let assistant_idx = hist.len() - 1;
            for (detail_idx, tc) in response.tool_calls.iter().enumerate() {
                let result = self.execute_tool(tc, &executor).await;
                let result_str = match &result {
                    Ok(s) => s.clone(),
                    Err(e) => format!("Error: {}", e),
                };
                if let Some(entry) = hist.get_mut(assistant_idx) {
                    if let Some(detail) = entry.tool_details.get_mut(detail_idx) {
                        detail.status = if result.is_ok() {
                            "completed".into()
                        } else {
                            "failed".into()
                        };
                        detail.result = Some(result_str.clone());
                    }
                }
                hist.push(ConversationEntry {
                    role: "user".into(),
                    content: format!("Tool '{}' returned:\n{}", tc.name, result_str),
                    timestamp: chrono::Utc::now().timestamp(),
                    tool_calls: vec![],
                    tool_details: vec![],
                });
            }
        }

        Err("Agent exceeded maximum tool call depth (10 iterations). The loop was terminated to prevent an infinite loop.".into())
    }

    async fn execute_tool(
        &self,
        call: &ToolCall,
        executor: &Option<Arc<dyn ToolExecutor>>,
    ) -> Result<String, String> {
        match call.name.as_str() {
            "send_file" => {
                let file_path = call
                    .arguments
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or("send_file: missing 'file_path' argument")?;
                let peer_id = call
                    .arguments
                    .get("peer_id")
                    .and_then(|v| v.as_str())
                    .ok_or("send_file: missing 'peer_id' argument")?;
                match self
                    .permission_store
                    .check_read_path(std::path::Path::new(file_path))
                {
                    crate::agent::permissions::PermissionCheck::Allowed => {}
                    crate::agent::permissions::PermissionCheck::Denied { reason } => {
                        return Err(format!("Permission denied: {}", reason));
                    }
                }
                match executor {
                    Some(exe) => exe.send_file(file_path, peer_id).await,
                    None => Err("P2P module not initialized. Cannot send files.".into()),
                }
            }
            "list_peers" => match executor {
                Some(exe) => exe.list_peers().await,
                None => Ok("No P2P module connected. You are offline.".into()),
            },
            "list_transfers" => {
                let status = call.arguments.get("status").and_then(|v| v.as_str());
                match executor {
                    Some(exe) => exe.list_transfers(status).await,
                    None => Ok("No P2P module connected.".into()),
                }
            }
            "get_own_peer_id" => match executor {
                Some(exe) => exe.get_own_peer_id().await,
                None => Err("P2P module not initialized.".into()),
            },
            "find_file" => {
                let pattern = call
                    .arguments
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or("find_file: missing 'pattern' argument")?;
                let dir = call.arguments.get("directory").and_then(|v| v.as_str());
                let ext = call.arguments.get("extension").and_then(|v| v.as_str());
                let effective_dir = if let Some(d) = dir {
                    if !d.is_empty() {
                        d
                    } else {
                        self.permission_store
                            .config()
                            .allowed_paths
                            .first()
                            .map(|s| s.as_str())
                            .ok_or(
                                "find_file: no allowed paths configured and no directory specified",
                            )?
                    }
                } else {
                    self.permission_store
                        .config()
                        .allowed_paths
                        .first()
                        .map(|s| s.as_str())
                        .ok_or(
                            "find_file: no allowed paths configured and no directory specified",
                        )?
                };
                match self
                    .permission_store
                    .check_read_path(std::path::Path::new(effective_dir))
                {
                    crate::agent::permissions::PermissionCheck::Allowed => {}
                    crate::agent::permissions::PermissionCheck::Denied { reason } => {
                        return Err(format!("Permission denied: {}", reason));
                    }
                }
                self.find_file_local(pattern, Some(effective_dir), ext)
                    .await
            }
            "read_file" => {
                let path = call
                    .arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("read_file: missing 'path' argument")?;
                match self
                    .permission_store
                    .check_read_path(std::path::Path::new(path))
                {
                    crate::agent::permissions::PermissionCheck::Allowed => {}
                    crate::agent::permissions::PermissionCheck::Denied { reason } => {
                        return Err(format!("Permission denied: {}", reason));
                    }
                }
                self.read_file_local(path).await
            }
            "list_directory" => {
                let path = call
                    .arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("list_directory: missing 'path' argument")?;
                match self
                    .permission_store
                    .check_read_path(std::path::Path::new(path))
                {
                    crate::agent::permissions::PermissionCheck::Allowed => {}
                    crate::agent::permissions::PermissionCheck::Denied { reason } => {
                        return Err(format!("Permission denied: {}", reason));
                    }
                }
                self.list_directory_local(path).await
            }
            "shell_exec" => {
                let command = call
                    .arguments
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or("shell_exec: missing 'command' argument")?;
                PermissionStore::sanitize_command(command)?;
                match self.permission_store.check_command(command) {
                    crate::agent::permissions::PermissionCheck::Allowed => {}
                    crate::agent::permissions::PermissionCheck::Denied { reason } => {
                        return Err(format!("Permission denied: {}", reason));
                    }
                }
                self.shell_exec_local(command).await
            }
            other => Err(format!("Unknown tool: {}", other)),
        }
    }

    async fn find_file_local(
        &self,
        pattern: &str,
        directory: Option<&str>,
        extension: Option<&str>,
    ) -> Result<String, String> {
        let search_dir = directory.ok_or("find_file: no search directory provided")?;
        let mut results = Vec::new();
        let walker = walkdir::WalkDir::new(search_dir).max_depth(5).into_iter();
        for entry in walker.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let pat = pattern.to_lowercase();
            if name.contains(&pat) || glob_match(&pat, &name) {
                if let Some(ext) = extension {
                    if !entry.path().extension().map(|e| e == ext).unwrap_or(false) {
                        continue;
                    }
                }
                results.push(entry.path().to_string_lossy().to_string());
                if results.len() >= 20 {
                    break;
                }
            }
        }
        if results.is_empty() {
            Ok("No files found matching the pattern.".into())
        } else {
            Ok(format!(
                "Found {} file(s):\n{}",
                results.len(),
                results.join("\n")
            ))
        }
    }

    async fn read_file_local(&self, path: &str) -> Result<String, String> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| format!("Cannot read file: {}", e))?;
        let max_len = 50000;
        if content.len() > max_len {
            Ok(format!(
                "{}\n... [truncated at {} characters]",
                &content[..max_len],
                max_len
            ))
        } else {
            Ok(content)
        }
    }

    async fn list_directory_local(&self, path: &str) -> Result<String, String> {
        let mut entries = tokio::fs::read_dir(path)
            .await
            .map_err(|e| format!("Cannot list directory: {}", e))?;
        let mut items = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Error reading directory: {}", e))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            let kind = if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                "DIR"
            } else {
                "FILE"
            };
            items.push(format!("[{}] {}", kind, name));
        }
        items.sort();
        Ok(items.join("\n"))
    }

    async fn shell_exec_local(&self, command: &str) -> Result<String, String> {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await
            .map_err(|e| format!("Shell exec failed: {}", e))?;
        let mut result = String::new();
        if !output.stdout.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        if result.is_empty() {
            result.push_str("(no output)");
        }
        Ok(result)
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
        let allowed_str = if allowed.is_empty() {
            "none (you cannot read any files yet)"
        } else {
            &allowed
        };
        let writable_str = if writable.is_empty() {
            "none (you cannot write files yet)"
        } else {
            &writable
        };
        let commands_str = if commands.is_empty() {
            "none (you cannot run shell commands)"
        } else {
            &commands
        };

        let mut prompt = format!(
            r#"You are the ADTP agent running locally on this machine. You manage files, execute commands, and send files to other computers over the internet.

You have access to the following tools:
- send_file: Send a file to another computer by peer ID (requires file_path and peer_id)
- list_peers: List known remote peers and their online status
- list_transfers: List recent file transfers (optionally filter by status: completed, failed, in_progress)
- get_own_peer_id: Get this computer's own peer ID
- find_file: Search for files on this machine
- read_file: Read the contents of a local file
- list_directory: List contents of a directory
- shell_exec: Execute a permitted shell command

When the user asks you to send a file to another computer:
1. First use list_peers to see available peers
2. If the target peer exists, use send_file with the file path and peer ID
3. After sending, tell the user the result

When the user asks for their peer ID, use get_own_peer_id.
When the user asks about recent transfers, use list_transfers.

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

fn glob_match(pattern: &str, name: &str) -> bool {
    if pattern == name {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// Returns pre-scripted responses in order; lets us drive the tool loop
    /// (tool call, then final answer) without any live provider.
    struct ScriptedProvider {
        responses: StdMutex<Vec<LlmResponse>>,
    }

    #[async_trait]
    impl Provider for ScriptedProvider {
        async fn send_message(
            &self,
            _messages: &[LlmMessage],
            _system_prompt: &str,
            _tools: &[ToolDefinition],
        ) -> Result<LlmResponse, String> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                return Err("no more scripted responses".into());
            }
            Ok(responses.remove(0))
        }

        fn provider_name(&self) -> &str {
            "scripted"
        }

        fn model_name(&self) -> &str {
            "scripted-1"
        }
    }

    struct RecordingExecutor {
        sent: StdMutex<Vec<(String, String)>>,
        fail: bool,
    }

    #[async_trait]
    impl ToolExecutor for RecordingExecutor {
        async fn send_file(&self, file_path: &str, peer_id: &str) -> Result<String, String> {
            self.sent
                .lock()
                .unwrap()
                .push((file_path.to_string(), peer_id.to_string()));
            if self.fail {
                Err("peer offline".into())
            } else {
                Ok(format!("queued {file_path} -> {peer_id}"))
            }
        }

        async fn list_peers(&self) -> Result<String, String> {
            Ok("[\"peer-a\"]".into())
        }

        async fn get_own_peer_id(&self) -> Result<String, String> {
            Ok("self-peer-id".into())
        }

        async fn list_transfers(&self, _status: Option<&str>) -> Result<String, String> {
            Ok("[]".into())
        }
    }

    fn response(content: &str, tool_calls: Vec<ToolCall>) -> LlmResponse {
        LlmResponse {
            content: content.into(),
            tool_calls,
            stop_reason: "end_turn".into(),
        }
    }

    fn send_file_call(path: &str, peer: &str) -> ToolCall {
        ToolCall {
            id: "call-1".into(),
            name: "send_file".into(),
            arguments: serde_json::json!({"file_path": path, "peer_id": peer}),
        }
    }

    /// Builds a runtime whose permissions allow exactly `allowed_dir`, with the
    /// given executor.
    fn runtime_with(
        responses: Vec<LlmResponse>,
        allowed_dir: &std::path::Path,
        executor: Arc<RecordingExecutor>,
    ) -> AgentRuntime {
        let mut runtime = AgentRuntime::new(
            AgentConfig::default(),
            PermissionConfig {
                allowed_paths: vec![allowed_dir.to_string_lossy().into_owned()],
                ..Default::default()
            },
        );
        runtime.set_tool_executor(executor);
        runtime.provider = Arc::new(Mutex::new(Some(Box::new(ScriptedProvider {
            responses: StdMutex::new(responses),
        }))));
        runtime
    }

    fn temp_file(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "adtp-agent-runtime-test-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("report.txt");
        std::fs::write(&file, b"hello").unwrap();
        (dir, file)
    }

    #[tokio::test]
    async fn test_send_message_records_tool_call_params_and_result() {
        let (dir, file) = temp_file("ok");
        let executor = Arc::new(RecordingExecutor {
            sent: StdMutex::new(vec![]),
            fail: false,
        });
        let path = file.to_string_lossy().into_owned();
        let runtime = runtime_with(
            vec![
                response("Sending the file.", vec![send_file_call(&path, "peer-42")]),
                response("Done — file is on its way.", vec![]),
            ],
            &dir,
            executor.clone(),
        );

        let text = runtime
            .send_message("send report.txt to peer-42")
            .await
            .unwrap();
        assert_eq!(text, "Done — file is on its way.");

        // The executor really ran with the model's arguments.
        let sent = executor.sent.lock().unwrap().clone();
        assert_eq!(sent, vec![(path.clone(), "peer-42".to_string())]);

        // The assistant turn carries full detail for the mobile companion.
        let conversation = runtime.conversation().await;
        let assistant = conversation
            .iter()
            .find(|e| e.role == "assistant" && !e.tool_details.is_empty())
            .expect("assistant entry with tool details");
        assert_eq!(assistant.tool_calls, vec!["send_file".to_string()]);
        let detail = &assistant.tool_details[0];
        assert_eq!(detail.id, "call-1");
        assert_eq!(detail.name, "send_file");
        assert_eq!(detail.status, "completed");
        assert_eq!(detail.params["file_path"], path.as_str());
        assert_eq!(detail.params["peer_id"], "peer-42");
        assert!(detail.result.as_deref().unwrap().contains("queued"));

        // Tool details survive the JSON round-trip used by the IPC socket.
        let json = serde_json::to_value(&conversation).unwrap();
        assert_eq!(json[1]["tool_details"][0]["params"]["peer_id"], "peer-42");
    }

    #[tokio::test]
    async fn test_send_message_marks_failed_tool_call() {
        let (dir, file) = temp_file("fail");
        let executor = Arc::new(RecordingExecutor {
            sent: StdMutex::new(vec![]),
            fail: true,
        });
        let path = file.to_string_lossy().into_owned();
        let runtime = runtime_with(
            vec![
                response("", vec![send_file_call(&path, "peer-42")]),
                response("Sorry, that failed.", vec![]),
            ],
            &dir,
            executor,
        );

        runtime.send_message("send report.txt").await.unwrap();

        let conversation = runtime.conversation().await;
        let detail = conversation
            .iter()
            .flat_map(|e| e.tool_details.iter())
            .next()
            .expect("tool detail");
        assert_eq!(detail.status, "failed");
        assert!(detail.result.as_deref().unwrap().contains("peer offline"));
    }
}
