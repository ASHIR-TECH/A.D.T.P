# Phase 10 — Embedded Agent Integration

Self-contained AI agent embedded in the ADTP desktop peer, activated by a single API key. The agent receives natural language instructions, searches and operates on local system files under a user-controlled permission model, executes ADTP operations as tools, and responds with results — all running locally.

## Architecture

```
User → Chat Panel (Desktop App) → Tauri Commands → AgentRuntime (adtp-peer-core) → LLM Provider
                                                            ↓
User ← Chat Panel (Desktop App) ← Tauri Commands ← AgentRuntime (adtp-peer-core) ← LLM Provider
                                                                   ↓
                                                    MCP System Tools (Python)
                                                    find_file, read_file, shell_exec, list_directory
                                                                   ↓
                                                    PermissionStore (permissions.toml)
```

## What's New

### Rust — `crates/adtp-peer-core/src/agent/`

| File | Purpose |
|------|---------|
| `mod.rs` | Module re-exports |
| `config.rs` | `AgentConfig` struct, TOML load/save at `~/.config/adtp/agent.toml` |
| `permissions.rs` | `PermissionStore` with path, command, and file-size checks; TOML load/save at `~/.config/adtp/permissions.toml` |
| `provider.rs` | `Provider` trait with `send_message()`, `LlmMessage`, `LlmResponse`, `ToolDefinition`, `ToolCall` |
| `anthropic.rs` | `AnthropicProvider` — Claude API via `POST /v1/messages` |
| `openai.rs` | `OpenAiProvider` — GPT API via `POST /v1/chat/completions` |
| `ollama.rs` | `OllamaProvider` — local Ollama via `/api/chat`, model availability check |
| `runtime.rs` | `AgentRuntime` — manages provider lifecycle, conversation history, system prompt builder |
| `task_router.rs` | `TaskRouter` — formats incoming ADTP tasks for the LLM, routes to runtime |
| `artifact_writer.rs` | `ArtifactWriter` — writes LLM output to temp files at `~/.local/share/adtp/artifacts/` |
| `queue.rs` | `PendingTaskQueue` — persists tasks for offline peers, fires on reconnect |

### Python — `crates/adtp-mcp/src/adtp_mcp/tools/`

| File | Purpose |
|------|---------|
| `find_file.py` | Filesystem search by name/glob/extension, respects `allowed_paths` |
| `read_file.py` | Read file contents with encoding detection, size limit, path permission check |
| `shell_exec.py` | Execute commands from strict allowlist, blocks dangerous commands |
| `list_directory.py` | List directory contents with metadata, respects `allowed_paths` |

### Desktop App Frontend — `desktop/src/`

| File | Purpose |
|------|---------|
| `pages/AgentSetup.js` | Three-step setup wizard (Provider → API Key → Permissions) |
| `pages/Permissions.js` | Full permission management screen |
| `components/ChatPanel.js` | Natural language chat panel (embedded in Dashboard sidebar) |
| `components/AgentStatusCard.js` | Agent status card in Settings |
| `components/PermissionItem.js` | Single permission row with toggle |

### Desktop App Backend — `desktop/src-tauri/src/`

| File | Purpose |
|------|---------|
| `agent_manager.rs` | `AgentManager` — wraps `AgentRuntime` in `Arc<Mutex<>>` shared state |
| `commands.rs` | 8 new `#[tauri::command]` handlers: `agent_get_status`, `agent_activate`, `agent_deactivate`, `agent_send_message`, `agent_get_permissions`, `agent_update_permissions`, `agent_get_conversation`, `agent_clear_conversation`, `get_home_dir` |

## Dependencies

- **Rust**: `reqwest` 0.12, `tokio-stream` 0.1, `async-trait` (workspace)
- **Python**: `stdlib` only (`pathlib`, `glob`, `subprocess`, `chardet` from Phase 3)

## Config Files

| File | Format | Purpose |
|------|--------|---------|
| `~/.config/adtp/agent.toml` | TOML | Provider, model, temperature, max_tokens, api_key, can_task_from |
| `~/.config/adtp/permissions.toml` | TOML | allowed_paths, writable_paths, denied_paths, allowed_commands, max_file_read_bytes, allow_hidden_files, allow_remote_tasks |

## Permission Model

| Permission | Default | What It Controls |
|------------|---------|------------------|
| `allowed_paths` | Empty | Directories the agent may read files from |
| `writable_paths` | Empty | Directories the agent may write files to |
| `allowed_commands` | Empty | Shell commands the agent may execute |
| `max_file_read_bytes` | 10 MB | Maximum single file size for read_file |
| `allow_hidden_files` | false | Whether dotfiles are accessible |
| `allow_remote_tasks` | true | Whether other peers can send tasks |

Blocked commands (always denied): `rm`, `sudo`, `chmod`, `chown`, `dd`, `mkfs`, `fdisk`, `mount`, `umount`, `passwd`, `useradd`, `usermod`, `groupadd`, `kill`, `pkill`, `reboot`, `shutdown`, `init`, `systemctl`.

## Provider Support

| Provider | Model | Offline | Auth |
|----------|-------|---------|------|
| Anthropic (Claude) | claude-sonnet-4-6 / claude-opus-4-6 | No | X-API-Key from keychain |
| OpenAI (GPT) | gpt-4o / gpt-4o-mini | No | Bearer token from keychain |
| Ollama (local) | Any installed model | Yes | None (local only) |

## Agent Tools

The embedded agent has access to ADTP protocol tools (`send_file`, `list_transfers`) plus system file tools (`find_file`, `read_file`, `list_directory`, `shell_exec`). Tool schemas are passed to the LLM as function/tool definitions.

## System Prompt

Assembled at runtime per session:
- Identity: peer ID and display name
- Capabilities: all registered tool names
- Permissions: allowed_paths, writable_paths, allowed_commands
- Optional: `system_prompt_override` from agent.toml

## Testing

### Rust unit tests (17)
```
cargo test -p adtp-peer-core -- agent
```
- Queue: enqueue/fire, cancel, pending-for-peer, expiry (4)
- Config: defaults, serialization roundtrip, path (3)
- Permissions: blocked commands, allowed commands, traversal, file size, hidden files, save/load roundtrip, defaults (10)

### Python tool tests (6)
```
cd crates/adtp-mcp && PYTHONPATH=src python3 -m pytest tests/
```
- find_file: denies empty allowed_paths, finds files in allowed path
- shell_exec: blocks rm, runs echo
- list_directory: denies without permission
- read_file: denies outside allowed_paths

### Desktop backend tests (11 + new agent)
```
cargo test -p adtp-desktop
```
All existing settings/tray/poller tests continue to pass. Agent commands tested indirectly through Rust type system.

## Usage

1. Open **Settings → Embedded Agent** in the Desktop App
2. Click **Add Agent** to start the three-step setup
3. Select a provider (Anthropic, OpenAI, or Ollama)
4. Enter API key (saved to OS keychain) or Ollama host
5. Grant permissions (directories, commands, toggles)
6. Click **Activate Agent**
7. Chat panel appears in Dashboard sidebar — type natural language instructions

## Key Design Decisions

- The agent lives as a library module in `adtp-peer-core` and runs inside the Desktop App process via Tauri commands — no separate daemon or HTTP server required for the desktop use case.
- REST API endpoints (`/api/v1/agent/*`) are defined in the Phase 10 spec but not implemented — they are reserved for the Phase 11 Mobile App which will communicate with the agent remotely.
- API key stored in OS keychain (`keyring` crate), never on disk.
- Permission model is opt-in — all paths and commands are denied by default.
- Python MCP tools are standalone scripts with no framework dependency — they can be called directly or integrated into an MCP server later.
