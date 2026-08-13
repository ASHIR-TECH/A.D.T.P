# ADTP — Mobile Port Handoff Document

> **Purpose:** This document is the primary context for building the mobile version of ADTP in a new repository. It captures the architecture, wire protocol, security model, agent system, and all moving parts of the existing desktop/server implementation so the mobile port can reuse or faithfully reimplement them.
>
> **Source repo:** `github.com/ASHIR-TECH/A.D.T.P` — branch `Phase-13.5`, tag `v0.1.0`, MIT license.
>
> **Hard requirement:** Do NOT regress the Phase A security posture (TOFU TLS, rendezvous Ed25519+HMAC auth, ECDH session keys, permission checks). Mobile must implement the same trust boundaries, or a documented, equivalent set.

---

## 1. Project Overview

ADTP (Advanced Data Transfer Protocol) is a secure, P2P-oriented file/artifact transfer and agent-communication protocol. A single Rust workspace implements:

| Component | Location | Role |
|---|---|---|
| `adtp-core` | `crates/adtp-core` | Types, wire protocol (de)serialization, integrity, encryption, sessions, TOFU trust, transfer primitives |
| `adtp-proto` | `crates/adtp-proto` | Protocol helpers (thin layer over adtp-core) |
| `adtp-client` | `crates/adtp-client` | Daemon + transfer queue + file receiver + folder watcher + audit log (client side) |
| `adtp-server` | `crates/adtp-server` | Linux "AI server": intake + dispatcher + push (server side) |
| `adtp-peer` / `adtp-peer-core` | `crates/adtp-peer`, `crates/adtp-peer-core` | Symmetric P2P engine, agent runtime, agent card, dual endpoint, NAT detection, rendezvous client, relay, subscribe, tasks, audit |
| `adtp-rendezvous` | `crates/adtp-rendezvous` | Public UDP rendezvous: peer table, auth tokens, STUN, wake |
| `adtp-cli` | `crates/adtp-cli` | REPL CLI + ~17 command modules |
| `adtp-api` | `packages/adtp-api` | Python FastAPI REST gateway (aiosqlite) |
| `adtp-mcp` | `packages/adtp-mcp` + `crates/adtp-mcp` | Python MCP server + bridge (tools over unix sockets) |
| `desktop` | `desktop/` | Tauri v2 desktop app (vanilla JS ESM) — the UI reference for mobile |
| `docs/` | specs, guides | `docs/spec/*` = protocol/security/schema specs; `docs/guides/*` = user guides |
| `phases/` | build docs | `ADTP_Phase0`–`13` build/readme docs — feature chronology |

Build system: Rust workspace root `Cargo.toml` (edition 2021), `pyproject.toml` for Python packages, `Makefile`, GitHub Actions in `.github/workflows/build.yml`.

---

## 2. Port Map

| Port | Env / constant | Purpose |
|---|---|---|
| **40001** | `ADTP_DEFAULT_PORT` | ADTP QUIC primary transfer port |
| **40002** | agent card port | MCP server / agent card |
| **40003** | `ADTP_API_PORT` | FastAPI REST gateway |
| **40004** | `RENDEZVOUS_DEFAULT_PORT` | Rendezvous UDP (default `udp://rendezvous.ashir.world:40004`; Phase 12 docs cite `rendezvous.adtp.network:40004`) |
| **443** | — | QoWT relay `wss://relay.ashir.world:443/adtp` (QUIC-over-WebSocket-over-TLS fallback) |
| **19302** | — | STUN fallback `stun1.l.google.com:19302` |

Unix domain sockets (POSIX-only — **mobile must replace these**):
- `/tmp/adtp-client.sock` — client daemon <-> services
- `/tmp/adtp-server.sock` — server daemon <-> services
- Used by the bridge layer connecting daemons <-> MCP <-> REST.

---

## 3. Architecture at a Glance

```
                Public Infrastructure
   rendezvous.ashir.world:40004 (UDP)   relay.ashir.world:443   STUN stun1.l.google.com:19302
                ^                                        ^
   registration / hole-punch / relay fallback           QoWT fallback

   Peer A (desktop / mobile)                     Peer B (desktop / AI server)
   adtp-peer-core (P2P + agent)                  adtp-peer-core or adtp-server
   QUIC listener :40001 + agent runtime          QUIC listener :40001 + agent runtime
     |  Tauri desktop UI / CLI /                     |  FastAPI REST :40003 (adtp-api)
     |  MCP server :40002                            |  MCP :40002
     +-- unix sockets (client/server)                +-- unix sockets
   ~/.config/adtp/  (keys, trust, configs, agents, audit.sqlite)
```

Data plane: **QUIC** (TLS 1.3) with ADTP packets on top. Control plane: rendezvous UDP for discovery, hole punching, and relay fallback. Management: unix sockets + REST + MCP on each node.

---

## 4. Wire Protocol Spec

### 4.1 Constants (`crates/adtp-core/src/types.rs`)

```
ADTP_MAGIC          = 0xAD7F1A00   (u32)
ADTP_VERSION        = 1            (u8)
ADTP_DEFAULT_PORT   = 40001        (u16)
ADTP_MAX_FILE_SIZE  = 10 GiB
ADTP_MAX_CONTEXT_SIZE = 1 MiB
AES_KEY_SIZE        = 32, AES_NONCE_SIZE = 12, AES_TAG_SIZE = 16
ED25519_SIG_SIZE    = 64, BLAKE3_HASH_SIZE = 32, PEER_ID_SIZE = 16
```

### 4.2 PacketHeader (`crates/adtp-core/src/protocol.rs`)

Serialized header: magic, version, transfer_type, priority, flags, session_id, transfer_id, sender_peer_id [16B], receiver_peer_id [16B], timestamp_ns, context_len, payload_len, nonce [12B], content_hash [32B = BLAKE3 of payload]. Validation: magic and version checked on decode (`validate()`). Companion payloads: FileMeta, ContextBlock, transfer history entries.

### 4.3 TransferType discriminants

```
Transfer=0x01  Ack=0x03  Error=0x04  Watch=0x05  Unwatch=0x06
Status=0x07  History=0x08  Hello=0x09  Ping=0x0A  Pong=0x0B
Subscribe=0x0C  Notify=0x0D  TaskRequest=0x0E  TaskUpdate=0x0F  ArtifactReady=0x10
PeerRegister=0x20  RendezvousRequest=0x21  RendezvousResponse=0x22  PunchReady=0x23
```

### 4.4 Priority

`Low=0x00, Normal=0x01 (default), Urgent=0x02, Critical=0x03`.

### 4.5 TransferStatus

`Pending`, `InProgress{bytes_sent, total_bytes}`, `Delivered`, `Failed{reason}`, `Cancelled`.

### 4.6 TransferSession lifecycle

Sessions carry a `session_id`, sender/receiver peer IDs, encryption context, and a transfer queue. Design references in `docs/spec/adtp-protocol-spec.md`, `packet-structure.md`, `transfer-types.md`, `context-block.md`. Retry/ack semantics via `Ack`/`Error` types.

---

## 5. Identity & Security Model

**This is the most important section to preserve in the mobile port.**

### 5.1 Identity
- `peer_id` = first 16 bytes of `BLAKE3(Ed25519 public key)`.
- Ed25519 keys per node (ed25519_dalek). No central CA.

### 5.2 Encryption
- **AES-256-GCM** payload encryption (32B key, 12B nonce, 16B tag).
- **ECDH X25519** handshake -> **HKDF-SHA256** -> `derive_session_key()` session key per peer session.
- **Ed25519** packet signing (64B signature) so integrity + origin are cryptographically verifiable.

### 5.3 Trust
- **TOFU (Trust On First Use):** peer certs recorded in `~/.config/adtp/trusted_certs.toml` (chmod 0o600). Subsequent connections verified against the stored fingerprint; mismatch => reject.
- TLS 1.3 via QUIC (quinn + rustls).

### 5.4 Rendezvous auth
- Registration requests are **Ed25519-signed** with a **±60s timestamp window**.
- **HMAC-SHA256** bearer tokens (`TOKEN_VALIDITY_SECS`) issued/validated for control messages.

### 5.5 Integrity
`crates/adtp-core/src/integrity.rs` — the mobile port should mirror these:
- `hash_bytes()` -> `[u8; 32]` (BLAKE3)
- `hash_file()` (async, 64 KiB read buffer)
- `verify_hash()`, `hash_to_hex()`
- `generate_keypair()`, `sign()`, `verify_signature()`
- `genesis_hash()` seeds the audit chain.

### 5.6 Audit chain (`crates/adtp-client/src/audit.rs`)
SQLite (rusqlite / Mutex) with three tables:
- `transfers` — per-transfer record incl. `signature`, `content_hash`, `integrity_ok`, `chain_hash`.
- `sessions` — `peer_addr`, `transfer_count`.
- `chain_state` — single row; `last_hash` initialized from `genesis_hash()`, updated to the BLAKE3 of each appended block -> tamper-evident chain.

### 5.7 Permission checks (agent)
`PermissionStore` with a `BLOCKED_COMMANDS` list, `sanitize_command()`, and `check_read_path()` using canonicalized paths to prevent escaping the allowed roots. Apply identical policy on mobile.

---

## 6. Connection Paths & NAT Traversal

Priority-ordered connection attempts:
1. `direct_ipv6`
2. `direct_ipv4`
3. `lan` (mDNS discovery)
4. `hole_punch` (rendezvous-assisted UDP/QUIC hole punching; `PunchReady` flow)
5. `relay` (rendezvous-provided relay)
6. `qowt` — QUIC-over-WebSocket-over-TLS via `wss://relay.ashir.world:443/adtp`

NAT types detected: `open`, `full_cone`, `port_restricted`, `symmetric`, `cgnat`. Implemented in `crates/adtp-peer-core/src/nat_detect.rs`; STUN fallback in `crates/adtp-rendezvous/src/stun.rs`. Rendezvous client: `crates/adtp-peer-core/src/rendezvous_client.rs`; relay: `relay.rs`.

---

## 7. Agent System (embedded agent + remote tasking)

### 7.1 Providers (`crates/adtp-peer-core/src/agent/`)
- `openai.rs`, `anthropic.rs`, `ollama.rs` (local models), behind a common `provider.rs` trait.

### 7.2 Tool loop
`AgentRuntime` runs the agent loop capped at **max 10 iterations** (`runtime.rs`). Each iteration: observe tool results -> decide next tool call -> execute -> return, until final answer.

### 7.3 ToolExecutor trait (`crates/adtp-peer-core/src/agent/runtime.rs`)

```rust
pub trait ToolExecutor: Send + Sync {
    async fn send_file(&self, file_path: &str, peer_id: &str) -> Result<String, String>;
    async fn list_peers(&self) -> Result<String, String>;
    async fn get_own_peer_id(&self) -> Result<String, String>;
    async fn list_transfers(&self, status: Option<&str>) -> Result<String, String>;
}
```

The desktop app implements this on `P2PModule` (`desktop/src-tauri/src/p2p.rs`) and wires it via `agent_manager.rs::set_tool_executor()`.

### 7.4 Local tools
`find_file`, `read_file`, `list_directory`, `shell_exec` — all gated through `PermissionStore`.

### 7.5 Remote tasking
`TaskRequest` -> `TaskUpdate` -> `ArtifactReady` packet flow. Task router + queue in `agent/task_router.rs` and `agent/queue.rs`; results written via `agent/artifact_writer.rs`.

### 7.6 Agent card
`agent_card.rs` advertises agent identity/capabilities (`docs/spec/agent-card-schema.md`, `a2a-transport-binding.md`).

### 7.7 Config files
- `~/.config/adtp/agent.toml` — single agent config (provider, model, system prompt).
- `~/.config/adtp/agents.toml` — multi-agent config.
- `~/.config/adtp/permissions.toml` — allowed roots / commands.
- Example files under `config/`.

---

## 8. Daemon & IPC

### 8.1 Client daemon (`crates/adtp-client/src/daemon.rs`, `ipc.rs`)
Owns the QUIC endpoint, transfer queue (`queue.rs`), file receiver (`receiver.rs`), folder watcher (`watcher.rs` + `notify.rs`). Exposes IPC over the client unix socket.

### 8.2 Server daemon (`crates/adtp-server/src/`)
`intake.rs` (accept files), `dispatcher.rs` (route to handlers), `push.rs` (outbound). Config in `config.rs`; audit mirror in `audit.rs`.

**Mobile port note:** unix sockets are POSIX-only. Replace with the platform IPC equivalent (e.g. localhost TCP, named pipes, or in-process modules) but keep the logical interfaces so MCP/REST bridges can move across.

---

## 9. REST API (`packages/adtp-api`, Python FastAPI)

- Routes: `/api/v1/{health, ready, queue, transfers, audit, watchers, sessions, tasks}`.
- **Bearer HMAC token auth**; health/ready paths exempt. Token must be >= 32 chars; configurable via `ADTP_API_*` env vars.
- Rate limiting via slowapi.
- aiosqlite persistence. Config example: `config/`; guide: `docs/guides/mcp-integration.md`, `docs/guides/agent-setup.md`.
- **Mobile frontend can call this REST API directly** — prefer this over unix-socket bridging on device.

---

## 10. MCP Server (`packages/adtp-mcp` + `crates/adtp-mcp`, Python)

- Listens on port **40002**.
- Tools: `send_file`, `receive_file`, `watch_folder`, `unwatch_folder`, `list_transfers`, `get_file_context`, `transfer_status`.
- Additional agent system tools (Python): `find_file`, `list_directory`, `read_file`, `shell_exec`.
- Bridge client talks to daemons over the unix sockets.

---

## 11. Desktop App (UI reference for mobile)

- Tauri v2, vanilla JavaScript ES modules, no framework.
- Pages: **Dashboard, History, Settings, AgentSetup, Permissions**.
- Components: `ChatPanel`, `Sidebar`, `TransferList`, `PeerCard`, `AgentCard`, etc.
- Plugins: dialog, fs, notification, store, shell.
- Tray icons: `tray-idle`, `tray-error`, `tray-sync`.
- Config dir: `~/.config/adtp`.
- `lib.rs` starts P2P on a **separate tokio runtime** and registers Tauri commands + the ToolExecutor.
- Phase 12 P2P commands (`desktop/src-tauri/src/p2p.rs`): `get_own_peer_id`, `get_connection_status`, `list_peers`, `send_file_to_peer`, `add_peer`, `remove_peer`, `get_own_nat_type`, `get_agent_status`; plus file push and transfer history.
- App icon source: `media/Logo1.png` (1024x1024). Icons regenerated via `cargo tauri icon media/Logo1.png`.

---

## 12. Phase Status (what shipped, feature chronology)

`phases/` build docs + `CHANGELOG.md` are the authoritative record. Summary:

- **Phase 0-9:** core protocol, security (TOFU, ECDH, rendezvous auth — Phase A hardening), client/server daemons, CLI, watcher, audit chain, REST gateway, MCP, docs/specs.
- **Phase 9.5:** agent card schema, A2A transport binding.
- **Phase 10:** embedded agent — providers (openai/anthropic/ollama), tool loop (<=10 iterations), `PermissionStore`, MCP system tools, agent/permissions config files.
- **Phase 11:** multi-agent config (`agents.toml`), task routing, artifact writing.
- **Phase 12:** P2P agent communication + desktop integration — `P2PModule`, `ToolExecutor` wiring, Tauri P2P commands, rendezvous/STUN/NAT-type reporting.
- **Phase 13:** CLI survey + polish; current branch `Phase-13.5`.

---

## 13. CI / Tooling

`.github/workflows/build.yml`:
- `lint`: ubuntu-24.04 + Tauri system deps, `cargo fmt` + `clippy`.
- `test-cli`: build + test `adtp-cli`, artifact `cli-linux`.
- `build-desktop` matrix: Linux (deb/appimage/rpm), Windows (msi), macOS (dmg).

---

## 14. Mobile Port — Key Decisions Checklist

1. **QUIC stack:** reuse quinn/rustls if a mobile runtime allows (Rust via UniFFI, or reimplement in Kotlin/Swift). QUIC/TLS 1.3 must stay.
2. **Replace unix sockets** (client/server IPC) with platform IPC; keep the MCP/REST bridge logic.
3. **Keep the crypto surface identical:** AES-256-GCM, X25519 ECDH -> HKDF-SHA256, Ed25519 signing, BLAKE3 hashing + chain. Do not weaken.
4. **Keep the wire format compatible** (`ADTP_MAGIC`/`ADTP_VERSION`/TransferType codes/PacketHeader layout) so mobile peers interop with desktop and servers.
5. **Agent runtime:** implement `ToolExecutor` (4 methods) and the <=10-iteration tool loop; gate all local tools with the equivalent of `PermissionStore`.
6. **Frontend:** call the FastAPI REST gateway (`/api/v1/*`) for management UI; use P2P commands for transfers.
7. **Config dir:** mirror `~/.config/adtp/` layout on the platform app-support directory.
8. **Rendezvous/relay/STUN defaults:** keep `rendezvous.ashir.world:40004`, `wss://relay.ashir.world:443/adtp`, `stun1.l.google.com:19302`.

---

## 15. Where to Look First in the Source

- `crates/adtp-core/src/types.rs` — constants, header types, statuses
- `crates/adtp-core/src/protocol.rs` — packet encode/decode, validate()
- `crates/adtp-core/src/integrity.rs` — BLAKE3 + Ed25519 helpers
- `crates/adtp-core/src/session.rs`, `encryption.rs`, `tofu.rs` — key derivation, AES-GCM, TOFU store
- `crates/adtp-peer-core/src/agent/runtime.rs` — agent loop + ToolExecutor trait
- `crates/adtp-peer-core/src/agent/permissions.rs` — PermissionStore policy
- `crates/adtp-client/src/audit.rs` — SQLite audit chain schema
- `desktop/src-tauri/src/p2p.rs` — desktop P2P integration + command surface
- `packages/adtp-api/src/adtp_api/` — REST routers, auth middleware
- `docs/spec/*` — protocol, security-model, agent-card-schema, a2a-transport-binding
- `config/` — example TOMLs for client/server/peer/mcp

