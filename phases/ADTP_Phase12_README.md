# Phase 12 — P2P Agent Communication & Desktop Integration

P2P agent-to-agent file transfer orchestrated by the local AI agent, embedded in the Tauri desktop app. Each PC is identified by an Ed25519-derived peer ID, discovers peers via a rendezvous server, and the agent executes transfers as tool calls.

## Architecture

```
User → Chat Panel → AgentRuntime (LLM) → ToolExecutor trait → P2PModule → QUIC/DualEndpoint → Target Peer
                                                                    ↓
                                                            Rendezvous Client
                                                    (rendezvous.adtp.network:40004)
                                                                    ↓
                                                            NatDetector (STUN)
```

## What's New

### Rust — `desktop/src-tauri/src/`

| File | Purpose |
|------|---------|
| `p2p.rs` | `P2PModule` – Ed25519 identity, QUIC listener, rendezvous registration, connection management, file push, transfer history, NAT type tracking |
| `agent_manager.rs` | `AgentManager.set_tool_executor()` – wires P2P capabilities into all agent runtimes |
| `commands.rs` | 5 P2P commands: `get_own_peer_id`, `get_connection_status`, `list_peers`, `send_file_to_peer`, `add_peer`, `remove_peer` + `get_own_nat_type`, `get_agent_status` |
| `lib.rs` | P2P init on startup via separate tokio runtime, command registration, executor wiring |

### Rust — `crates/adtp-peer-core/src/agent/`

| File | Purpose |
|------|---------|
| `runtime.rs` | `ToolExecutor` trait (4 methods: `send_file`, `list_peers`, `get_own_peer_id`, `list_transfers`) + tool execution loop (max 10 iterations) + local tools (`find_file`, `read_file`, `list_directory`, `shell_exec`) with permission checks |

### Desktop Frontend — `desktop/src/`

| File | Purpose |
|------|---------|
| `pages/Settings.js` | My Peer ID card with copy, peer management (add/remove), NAT type, connection status, agent active/total count |
| `pages/Dashboard.js` | "Send to Peer" button, file picker modal, Known Peers widget, P2P transfers in event table (gold left border) + activity widget |
| `components/ChatPanel.js` | Collapsible tool call steps (wrench icon, tool name, expandable params) |
| `styles.css` | `.peer-id-display`, `.peer-select`, `.peer-item`, `.peer-remove-btn`, `.chat-tool-call` styles |

## P2P Module (`P2PModule`)

| Aspect | Detail |
|--------|--------|
| Identity | Ed25519 keypair loaded/generated at `~/.config/adtp/identity.key` |
| Transport | QUIC via `DualEndpoint` on port 40001 |
| Peer Discovery | Registers with `rendezvous.adtp.network:40004`, re-registers every 240s |
| NAT Detection | STUN via `NatDetector` against Google's STUN servers |
| Peer Storage | `~/.config/adtp/peers.toml` (TOML with `[[peer]]` entries) |
| Accept Loop | Background `tokio::spawn` loop accepting incoming QUIC connections |
| Transfer Tracking | In-memory `transfer_history` vec of completed/failed sends |

## Agent Tools

| Tool | Description | Permission Check |
|------|-------------|-----------------|
| `send_file` | Send a file to another peer by peer ID | Path must be in `allowed_paths` |
| `list_peers` | List known remote peers | None |
| `list_transfers` | List recent transfers (optionally filter by status) | None |
| `get_own_peer_id` | Get this computer's peer ID | None |
| `find_file` | Search local filesystem by name/glob | Directory must be in `allowed_paths` |
| `read_file` | Read contents of a local file | Path must be in `allowed_paths` |
| `list_directory` | List contents of a directory | Path must be in `allowed_paths` |
| `shell_exec` | Execute a permitted shell command | Command must be in `allowed_commands` |

## Tauri Commands

| Command | Returns | Purpose |
|---------|---------|---------|
| `get_own_peer_id` | `String` | Hex-encoded Ed25519 peer ID |
| `get_connection_status` | `ConnectionStatus` | Rendezvous registered, connected peers count, endpoint port |
| `get_own_nat_type` | `String` | Detected NAT type (Open, FullCone, PortRestricted, Symmetric, etc.) |
| `get_agent_status` | `AgentStatusInfo` | Agent count, active count, per-agent states |
| `list_peers` | `Vec<PeerEntry>` | All known peers from `peers.toml` |
| `send_file_to_peer` | `String` | Send file via P2P push handler |
| `add_peer` | `bool` | Add peer to `peers.toml` |
| `remove_peer` | `bool` | Remove peer from `peers.toml` |

## Data Flow — Agent File Transfer

```
1. User: "send report.pdf to PC-456"
2. AgentRuntime.send_message() receives text
3. LLM returns tool_call: send_file(file_path="/home/user/report.pdf", peer_id="PC-456")
4. execute_tool() checks permissions (path in allowed_paths)
5. P2PModule.send_file_to_peer() called
   a. connect_to_peer() — checks connections map, then peers.toml
   b. QUIC connect to peer's host:port
   c. PushHandler.push() — encrypts + sends file
   d. TransferRecord recorded in transfer_history
6. Result fed back to LLM
7. LLM responds to user with confirmation
```

## Config Files

| File | Format | Purpose |
|------|--------|---------|
| `~/.config/adtp/identity.key` | Binary | Ed25519 keypair |
| `~/.config/adtp/peers.toml` | TOML | Peer entries with peer_id, host, port, nickname, trust_level |

## Dependencies Added

| Crate | Version | Used In |
|-------|---------|---------|
| `adtp-core` | workspace | P2P types, transfer, NAT types |
| `tracing` | workspace | P2P logging |
| `hex` | workspace | Peer ID hex encoding |
| `async-trait` | workspace | ToolExecutor trait |
| `rustls` | workspace | CryptoProvider install |
| `uuid` | workspace | Session ID for transfers |

## Key Design Decisions

- P2P networking embedded directly in Tauri app (not separate daemon) for single-process simplicity and direct access to AgentManager.
- `ToolExecutor` trait defined in `adtp-peer-core`, implemented by `P2PModule` in desktop binary — clean separation of concerns.
- Separate tokio runtime for P2P init (`std::mem::forget`-leaked) to avoid `Handle::current()` panics during app startup.
- Identity loaded/generated on first launch — deterministic peer ID across restarts.
- Peers configured manually in Settings UI + discovered via rendezvous server — no automatic LAN discovery.
- Transfer history tracked in-memory only (not persisted) — ephemeral per-session.

## Crates.io Release

The `adtp-cli` crate has been published to crates.io as `adtp-cli` v0.1.0, installable via:
```bash
cargo install adtp-cli
```

## Build Verification

```bash
cargo check -p adtp-desktop          # 0 errors, 0 warnings
cargo test -p adtp-cli               # 16/16 passed
cargo test -p adtp-peer-core -- agent # agent tests pass
```
