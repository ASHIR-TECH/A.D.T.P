use crate::agent::anthropic::AnthropicProvider;
use crate::agent::base::BaseProvider;
use crate::agent::config::AgentConfig;
use crate::agent::ollama::OllamaProvider;
use crate::agent::openai::OpenAiProvider;
use crate::agent::permissions::{PermissionConfig, PermissionStore};
use crate::agent::provider::{LlmMessage, LlmResponse, Provider, ToolCall, ToolDefinition};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub enum AgentState {
    Inactive,
    Active,
    Error(String),
}

#[allow(clippy::double_must_use)]
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn send_file(&self, file_path: &str, peer_id: &str) -> Result<String, String>;
    async fn list_peers(&self) -> Result<String, String>;
    async fn get_own_peer_id(&self) -> Result<String, String>;
    async fn list_transfers(&self, status: Option<&str>) -> Result<String, String>;

    /// Stage a file in a paired phone's inbox (the phone polls and downloads).
    async fn send_file_to_device(
        &self,
        file_path: &str,
        device_id: &str,
    ) -> Result<String, String> {
        let _ = (file_path, device_id);
        Err("Sending to paired devices is not available".into())
    }

    /// Send a chat message to a paired phone (durable inbox + live relay push).
    async fn send_message_to_device(&self, text: &str, device_id: &str) -> Result<String, String> {
        let _ = (text, device_id);
        Err("Messaging paired devices is not available".into())
    }

    /// List paired phones with identity, presence and telemetry. The agent uses
    /// this to answer "what devices are connected?" instead of guessing.
    async fn list_devices(&self) -> Result<String, String> {
        self.list_peers().await
    }
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
                name: "send_file_to_device".into(),
                description: "Send a local file to a paired phone (MARS). The phone downloads it from its inbox and it also appears in the phone's chat. Omit `device_id` when only one phone is paired.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string", "description": "Absolute path (or ~/path) to the file on this machine"},
                        "device_id": {"type": "string", "description": "ID of the paired phone from list_devices. Optional when only one phone is paired."},
                    },
                    "required": ["file_path"],
                }),
            },
            ToolDefinition {
                name: "send_message_to_device".into(),
                description: "Send a chat message to a paired phone (MARS). Use this whenever the user asks you to message, text, tell, or reply to their phone. Omit `device_id` when only one phone is paired.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": {"type": "string", "description": "Message text to deliver to the phone"},
                        "device_id": {"type": "string", "description": "ID of the paired phone from list_devices. Optional when only one phone is paired."},
                    },
                    "required": ["text"],
                }),
            },
            ToolDefinition {
                name: "list_devices".into(),
                description: "List the paired phones (MARS) with their name, device_id, online state, OS, battery, CPU and RAM. ALWAYS call this when the user asks about connected devices, paired phones, or a device by name.".into(),
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
                description: "Search the local filesystem for a file by name or pattern. By default this searches the user's Downloads, Documents, Pictures, Desktop and the phone-upload intake. Pass `directory` to search one folder.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string", "description": "Filename or glob pattern, e.g. image.json or *.png"},
                        "directory": {"type": "string", "description": "Optional absolute directory (or ~/path) to search instead of the default folders"},
                        "extension": {"type": "string", "description": "Optional file extension filter, e.g. png or .png"},
                    },
                    "required": ["pattern"],
                }),
            },
            ToolDefinition {
                name: "read_file".into(),
                description: "Read a local UTF-8 text file. For binary files (images, PDFs, archives) it returns metadata instead of bytes so you can still confirm and forward the file.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Absolute path or ~/path to the file"},
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
                        "path": {"type": "string", "description": "Absolute path or ~/path to the directory"},
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
                let file_path = crate::agent::permissions::expand_path(file_path)
                    .to_string_lossy()
                    .to_string();
                match self
                    .permission_store
                    .check_read_path(std::path::Path::new(&file_path))
                {
                    crate::agent::permissions::PermissionCheck::Allowed => {}
                    crate::agent::permissions::PermissionCheck::Denied { reason } => {
                        return Err(format!("Permission denied: {}", reason));
                    }
                }
                match executor {
                    Some(exe) => exe.send_file(&file_path, peer_id).await,
                    None => Err("P2P module not initialized. Cannot send files.".into()),
                }
            }
            "list_peers" => match executor {
                Some(exe) => exe.list_peers().await,
                None => Ok("No P2P module connected. You are offline.".into()),
            },
            "send_file_to_device" => {
                let file_path = call
                    .arguments
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or("send_file_to_device: missing 'file_path' argument")?;
                // Optional: the desktop picks the only paired phone when the
                // model omits it, which is the common "send it to my phone" case.
                let device_id = call
                    .arguments
                    .get("device_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let file_path = crate::agent::permissions::expand_path(file_path)
                    .to_string_lossy()
                    .to_string();
                match self
                    .permission_store
                    .check_read_path(std::path::Path::new(&file_path))
                {
                    crate::agent::permissions::PermissionCheck::Allowed => {}
                    crate::agent::permissions::PermissionCheck::Denied { reason } => {
                        return Err(format!("Permission denied: {}", reason));
                    }
                }
                match executor {
                    Some(exe) => exe.send_file_to_device(&file_path, &device_id).await,
                    None => Err("Gateway client is not initialized. Cannot send files.".into()),
                }
            }
            "send_message_to_device" => {
                let text = call
                    .arguments
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or("send_message_to_device: missing 'text' argument")?;
                let device_id = call
                    .arguments
                    .get("device_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                match executor {
                    Some(exe) => exe.send_message_to_device(text, &device_id).await,
                    None => {
                        Err("Gateway client is not initialized. Cannot message devices.".into())
                    }
                }
            }
            "list_devices" => match executor {
                Some(exe) => exe.list_devices().await,
                None => Ok(
                    "No gateway client is connected, so no paired devices are available.".into(),
                ),
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
                let dir = call
                    .arguments
                    .get("directory")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|d| !d.is_empty());
                let ext = call.arguments.get("extension").and_then(|v| v.as_str());

                let roots: Vec<std::path::PathBuf> = match dir {
                    Some(d) => vec![crate::agent::permissions::expand_path(d)],
                    None => self
                        .permission_store
                        .config()
                        .allowed_paths
                        .iter()
                        .map(|p| crate::agent::permissions::expand_path(p))
                        .collect(),
                };
                if roots.is_empty() {
                    return Err(
                        "find_file: no allowed paths configured and no directory specified".into(),
                    );
                }

                let mut allowed_roots = Vec::new();
                let mut denied_reasons = Vec::new();
                for root in roots {
                    match self.permission_store.check_read_path(&root) {
                        crate::agent::permissions::PermissionCheck::Allowed => {
                            allowed_roots.push(root)
                        }
                        crate::agent::permissions::PermissionCheck::Denied { reason } => {
                            denied_reasons.push(format!("{}: {reason}", root.display()))
                        }
                    }
                }
                if allowed_roots.is_empty() {
                    return Err(format!("Permission denied: {}", denied_reasons.join("; ")));
                }
                self.find_file_local(pattern, &allowed_roots, ext).await
            }
            "read_file" => {
                let path = call
                    .arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("read_file: missing 'path' argument")?;
                let path = crate::agent::permissions::expand_path(path);
                match self.permission_store.check_read_path(&path) {
                    crate::agent::permissions::PermissionCheck::Allowed => {}
                    crate::agent::permissions::PermissionCheck::Denied { reason } => {
                        return Err(format!("Permission denied: {}", reason));
                    }
                }
                self.read_file_local(&path.to_string_lossy()).await
            }
            "list_directory" => {
                let path = call
                    .arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("list_directory: missing 'path' argument")?;
                let path = crate::agent::permissions::expand_path(path);
                match self.permission_store.check_read_path(&path) {
                    crate::agent::permissions::PermissionCheck::Allowed => {}
                    crate::agent::permissions::PermissionCheck::Denied { reason } => {
                        return Err(format!("Permission denied: {}", reason));
                    }
                }
                self.list_directory_local(&path.to_string_lossy()).await
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
        directories: &[std::path::PathBuf],
        extension: Option<&str>,
    ) -> Result<String, String> {
        let wanted_ext = extension
            .map(|ext| ext.trim().trim_start_matches('.').to_lowercase())
            .filter(|ext| !ext.is_empty());
        let pat = pattern.trim().to_lowercase();
        if pat.is_empty() {
            return Err("find_file: pattern is empty".into());
        }

        // A full path (or a path that already exists) is the fastest answer,
        // but only when it sits inside one of the permission-checked roots.
        // Without this check `pattern: "/etc/passwd"` would bypass the roots
        // that execute_tool just validated.
        let direct = crate::agent::permissions::expand_path(pattern);
        if direct.is_file() && path_is_under_any(&direct, directories) {
            return Ok(format!(
                "Found 1 file(s):\n{}",
                describe_file_line(&direct).await
            ));
        }

        const SKIP_DIRS: &[&str] = &[
            "node_modules",
            "target",
            "venv",
            ".venv",
            "__pycache__",
            ".git",
            ".cache",
            "AppData",
            "Library",
        ];
        let mut results: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for root in directories {
            if !root.exists() {
                continue;
            }
            // Searching a file's parent when the caller passed a file root.
            let search_root = if root.is_dir() {
                root.clone()
            } else if let Some(parent) = root.parent() {
                parent.to_path_buf()
            } else {
                continue;
            };
            let walker = walkdir::WalkDir::new(&search_root)
                .max_depth(6)
                .into_iter()
                .filter_entry(|entry| {
                    if entry.depth() == 0 {
                        return true;
                    }
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                        return false;
                    }
                    true
                });
            for entry in walker.filter_map(|e| e.ok()) {
                if !entry.file_type().is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_lowercase();
                let matches = name.contains(&pat) || glob_match(&pat, &name);
                if !matches {
                    continue;
                }
                if let Some(ref ext) = wanted_ext {
                    let entry_ext = entry
                        .path()
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase());
                    if entry_ext.as_deref() != Some(ext.as_str()) {
                        continue;
                    }
                }
                let path = entry.path().to_path_buf();
                let key = path.to_string_lossy().to_string();
                if !seen.insert(key) {
                    continue;
                }
                results.push(describe_file_line(&path).await);
                if results.len() >= 30 {
                    break;
                }
            }
            if results.len() >= 30 {
                break;
            }
        }

        if results.is_empty() {
            Ok(format!(
                "No files found matching \"{pattern}\" in: {}",
                directories
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        } else {
            Ok(format!(
                "Found {} file(s) (newest match first is not guaranteed; pick the path that fits):\n{}",
                results.len(),
                results.join("\n")
            ))
        }
    }

    async fn read_file_local(&self, path: &str) -> Result<String, String> {
        let file_path = std::path::Path::new(path);
        let metadata = tokio::fs::metadata(file_path)
            .await
            .map_err(|e| format!("Cannot read file: {}", e))?;
        if !metadata.is_file() {
            return Err(format!("Not a regular file: {path}"));
        }
        if !self.permission_store.file_size_allowed(metadata.len()) {
            return Ok(format!(
                "File is too large to read inline: {path}\nSize: {}\nLimit: {}\n\
                 The file still exists and can be sent with send_file_to_device.",
                human_size(metadata.len()),
                human_size(self.permission_store.config().max_file_read_bytes),
            ));
        }

        let bytes = tokio::fs::read(file_path)
            .await
            .map_err(|e| format!("Cannot read file: {}", e))?;
        match String::from_utf8(bytes) {
            Ok(content) => {
                let max_chars = 50_000;
                if content.chars().count() > max_chars {
                    let truncated: String = content.chars().take(max_chars).collect();
                    Ok(format!(
                        "{truncated}\n... [truncated at {max_chars} characters]"
                    ))
                } else {
                    Ok(content)
                }
            }
            Err(error) => {
                // Images/PDFs/archives are not UTF-8. Returning readable
                // metadata (instead of an error) is what lets the agent confirm
                // the attachment and forward it to the phone.
                let bytes = error.into_bytes();
                Ok(describe_binary(path, metadata.len(), &bytes))
            }
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
            r#"You are the ADTP agent running locally on this machine. You manage files, execute commands, and send files to other computers and paired phones over the internet or LAN.

You have access to the following tools:
- send_file: Send a file to another computer by peer ID (requires file_path and peer_id)
- send_file_to_device: Send a file to a paired phone (MARS). Requires file_path; device_id is optional when only one phone is paired
- send_message_to_device: Send a chat message to a paired phone (MARS). Requires text; device_id is optional when only one phone is paired
- list_devices: List paired phones (name, device_id, online state, OS, battery, CPU and RAM)
- list_peers: List known remote peers and the paired phones (device_id + online status)
- list_transfers: List recent file transfers (optionally filter by status: completed, failed, in_progress)
- get_own_peer_id: Get this computer's own peer ID
- find_file: Search the user's Downloads, Documents, Pictures, Desktop and the phone-upload intake
- read_file: Read a text file; for images/PDFs/archives it returns metadata
- list_directory: List contents of a directory
- shell_exec: Execute a permitted shell command

When the user asks to send a file to another computer:
1. First use list_peers to see available peers
2. If the target peer exists, use send_file with the file path and peer ID
3. After sending, tell the user the result

When the user asks to send a file to their phone / MARS / this app:
1. Use list_devices (or list_peers) to find the paired phone (device_id)
2. Use send_file_to_device with the absolute file_path; omit device_id when only one phone is paired
3. The phone downloads it from its inbox and can also tap it in the chat. Tell the user the filename and that it is ready on the phone.

When the user asks about connected devices, paired phones, MARS, or a device by name (for example "Kali"):
1. ALWAYS call list_devices first. Its output is the source of truth for names, ids, online state, battery, CPU and RAM.
2. Answer using only devices from that output. Never invent a device and never claim you cannot see them.
3. If the user then asks to message or send something, use send_message_to_device and/or send_file_to_device with the matching device_id (omit it when only one is paired).

When the user asks you to message, text, tell, or reply to a phone:
1. Use list_devices to resolve the target (or omit device_id when only one is paired).
2. Call send_message_to_device with the exact text the user wants delivered.
3. If they also attached or named a file, call send_file_to_device in the same turn.

When the user says "send me", "send to my phone", "attach it", or references a file they just attached:
- Files attached from the phone are saved in the upload intake and are listed in the prompt.
- Use send_file_to_device to send them back (for example an image the user wants returned).
- Images are binary: read_file returns name/MIME/size, which is enough to confirm the file; do not claim to describe pixel contents unless a tool provides them.

When the user asks to find a file by name (for example "I want image.json"):
1. Use find_file with the filename or pattern; it searches the common user folders by default
2. Pick the matching absolute path, and use read_file first if you need to confirm JSON/text contents
3. If the user asked to send it, call send_file_to_device in the same turn

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

/// True when `path` canonicalises to a location inside one of `roots`.
fn path_is_under_any(path: &std::path::Path, roots: &[std::path::PathBuf]) -> bool {
    let Ok(candidate) = path.canonicalize() else {
        return false;
    };
    roots.iter().any(|root| {
        root.canonicalize()
            .map(|canonical| candidate.starts_with(canonical))
            .unwrap_or(false)
    })
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// MIME for text formats that magic-byte sniffers (`infer`) do not identify.
fn mime_from_extension(path: &std::path::Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    Some(match ext.as_str() {
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "csv" => "text/csv",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "ts" => "text/typescript",
        "rs" => "text/rust",
        "py" => "text/x-python",
        "sh" => "text/x-shellscript",
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "zip" => "application/zip",
        "gz" | "tgz" => "application/gzip",
        "tar" => "application/x-tar",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "heic" => "image/heic",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "apk" => "application/vnd.android.package-archive",
        "ipa" => "application/octet-stream",
        _ => return None,
    })
}

fn file_mime(path: &std::path::Path, bytes: &[u8]) -> String {
    if let Some(kind) = infer::get(bytes) {
        return kind.mime_type().to_string();
    }
    mime_from_extension(path)
        .unwrap_or("application/octet-stream")
        .to_string()
}

async fn describe_file_line(path: &std::path::Path) -> String {
    let size = tokio::fs::metadata(path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    // Sniff only the first 8 KiB; the file may be a multi-GB video.
    let mut head = vec![0u8; 8192];
    let read = match tokio::fs::File::open(path).await {
        Ok(mut file) => file.read(&mut head).await.unwrap_or(0),
        Err(_) => 0,
    };
    head.truncate(read);
    format!(
        "{} [{}] ({})",
        path.to_string_lossy(),
        file_mime(path, &head),
        human_size(size)
    )
}

fn describe_binary(path: &str, size: u64, bytes: &[u8]) -> String {
    let path = std::path::Path::new(path);
    let mime = file_mime(path, bytes);
    format!(
        "Binary file: {}\nPath: {}\nMIME: {}\nSize: {}\n\
         This file is not UTF-8 text, so its bytes are not shown. The file exists \
         and can be forwarded with send_file or send_file_to_device.",
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".into()),
        path.to_string_lossy(),
        mime,
        human_size(size)
    )
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

    #[tokio::test]
    async fn test_find_file_searches_every_allowed_root() {
        let base =
            std::env::temp_dir().join(format!("adtp-agent-find-test-{}", std::process::id()));
        let downloads = base.join("Downloads");
        let pictures = base.join("Pictures");
        std::fs::create_dir_all(&downloads).unwrap();
        std::fs::create_dir_all(&pictures).unwrap();
        let wanted = pictures.join("image.json");
        std::fs::write(&wanted, br#"{"ok":true}"#).unwrap();

        let runtime = AgentRuntime::new(
            AgentConfig::default(),
            PermissionConfig {
                allowed_paths: vec![
                    downloads.to_string_lossy().into_owned(),
                    pictures.to_string_lossy().into_owned(),
                ],
                ..Default::default()
            },
        );
        let result = runtime
            .find_file_local("image.json", &[downloads, pictures], None)
            .await
            .unwrap();

        assert!(result.contains("image.json"));
        assert!(result.contains("application/json"));
    }

    #[tokio::test]
    async fn test_find_file_rejects_direct_path_outside_allowed_roots() {
        let base =
            std::env::temp_dir().join(format!("adtp-agent-find-scope-test-{}", std::process::id()));
        let allowed = base.join("allowed");
        std::fs::create_dir_all(&allowed).unwrap();
        let secret = base.join("secret.txt");
        std::fs::write(&secret, b"do not expose").unwrap();

        let runtime = AgentRuntime::new(
            AgentConfig::default(),
            PermissionConfig {
                allowed_paths: vec![allowed.to_string_lossy().into_owned()],
                ..Default::default()
            },
        );
        let result = runtime
            .find_file_local(&secret.to_string_lossy(), &[allowed], None)
            .await
            .unwrap();

        assert!(result.starts_with("No files found"));
    }

    #[tokio::test]
    async fn test_read_file_describes_binary_instead_of_failing() {
        let base =
            std::env::temp_dir().join(format!("adtp-agent-binary-test-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let image = base.join("photo.png");
        // Minimal PNG magic + payload; enough for MIME sniffing.
        std::fs::write(&image, b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR").unwrap();

        let runtime = AgentRuntime::new(
            AgentConfig::default(),
            PermissionConfig {
                allowed_paths: vec![base.to_string_lossy().into_owned()],
                ..Default::default()
            },
        );
        let result = runtime
            .read_file_local(&image.to_string_lossy())
            .await
            .unwrap();

        assert!(result.contains("Binary file"));
        assert!(result.contains("image/png"));
        assert!(result.contains("send_file_to_device"));
    }

    #[tokio::test]
    async fn test_read_file_truncates_multibyte_text_safely() {
        let base =
            std::env::temp_dir().join(format!("adtp-agent-utf8-test-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let text = base.join("unicode.txt");
        std::fs::write(&text, "é".repeat(60_000)).unwrap();

        let runtime = AgentRuntime::new(
            AgentConfig::default(),
            PermissionConfig {
                allowed_paths: vec![base.to_string_lossy().into_owned()],
                ..Default::default()
            },
        );
        let result = runtime
            .read_file_local(&text.to_string_lossy())
            .await
            .unwrap();

        assert!(result.contains("truncated at 50000 characters"));
    }

    #[test]
    fn test_system_prompt_mentions_phone_files_and_find() {
        let runtime = AgentRuntime::new(AgentConfig::default(), PermissionConfig::default());
        let prompt = runtime.build_system_prompt();
        assert!(prompt.contains("send_file_to_device"));
        assert!(prompt.contains("find_file"));
        assert!(prompt.contains("phone"));
    }
}
