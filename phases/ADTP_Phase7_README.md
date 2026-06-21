# ADTP Phase 7 — Peer Identity & Symmetric Protocol

## Overview
Phase 7 introduces the **Peer Identity & Symmetric Protocol** layer, transforming ADTP from an asymmetric client-server model into a symmetric peer-to-peer transfer protocol. Every node now has a persistent cryptographic identity, mutual trust management, tamper-evident audit, and a QUIC-based dual endpoint that simultaneously listens and dials.

## Architecture

```
adtp-peer (binary)
└── adtp-peer-core (library)
    ├── identity.rs      — Ed25519 key generation, persistence, PeerIdentity derivation
    ├── trust.rs         — TrustStore (peers.toml: add, remove, list, is_allowed)
    ├── audit.rs         — AuditLog (SQLite + blake3 chain: log, verify, tamper detection)
    ├── intake.rs        — FileIntake (decrypt, verify, write to intake dir)
    ├── push.rs          — PushHandler (encrypt, sign, send file via QUIC)
    └── dual_endpoint.rs — Single Quinn Endpoint for both accept() and connect()
```

### Process Model
Phase 7 adds a new independent process: the **adtp-peer** daemon. Four processes now exist:

| Process | Role |
|---|---|
| `adtp-client` (daemon) | Legacy client — spawns per-user daemon |
| `adtp-server` (daemon) | Legacy server — listens for daemon connections |
| `adtp-peer` (daemon) | **NEW** — symmetric peer daemon (Phase 7) |
| `adtp-api` (Flask) | REST API (unchanged) |
| `desktop` (Tauri) | GUI (unchanged, + peers section) |

## New Crates

### `crates/adtp-peer-core` (library)
Shared logic for the symmetric peer model, reusable by both `adtp-peer` and future tooling.

| Module | File | Description |
|---|---|---|
| `PeerIdentityKey` | `identity.rs` | Loads or generates an Ed25519 signing key. Peer ID = `blake3(public_key)[:PEER_ID_SIZE]`. Key file stored at `~/.config/adtp/identity.key` with `chmod 600`. |
| `TrustStore` | `trust.rs` | Reads/writes `~/.config/adtp/peers.toml`. Manages `PeerEntry` list with `TrustLevel` (Trusted, Unverified, Blocked). Methods: `load`, `save`, `add_peer`, `remove_peer`, `set_trust`, `list_peers`, `get_peer`, `trust_level`, `is_allowed`. |
| `AuditLog` | `audit.rs` | SQLite-backed tamper-evident audit chain using blake3 hashes. Logs `sender_peer_id`, `receiver_peer_id`, `transfer_type`, `filename`, etc. `verify_chain()` detects tampering. |
| `FileIntake` | `intake.rs` | Peer-agnostic file receiver: decrypts, verifies content hash, writes to intake directory. Ported from `adtp-server`. |
| `PushHandler` | `push.rs` | Peer-agnostic file pusher: hashes, encrypts, signs, sends via QUIC `AdtpConnection`. |
| `DualEndpoint` | `dual_endpoint.rs` | Single Quinn `Endpoint` that both accepts incoming connections and dials outbound. Uses std `UdpSocket`, self-signed TLS certs with `SkipCertVerification`. |

### `crates/adtp-peer` (binary)
The peer daemon binary. Entry point at `crates/adtp-peer/src/main.rs`.

```bash
adtp-peer --listen-host 0.0.0.0 --listen-port 40001 --intake-dir /path/to/intake
```

On startup:
1. Loads or generates `PeerIdentityKey` from `~/.config/adtp/identity.key`
2. Loads `TrustStore` from `~/.config/adtp/peers.toml`
3. Binds `DualEndpoint` to the configured address
4. Opens `AuditLog` at `~/.config/adtp/peer-audit.db`
5. Enters accept loop — for each accepted connection:
   - Starts audit session
   - Processes incoming packets in a nested loop:
     - `TransferType::Transfer`: checks `sender_peer_id` against trust store → rejects untrusted → decrypts via `FileIntake` → logs to audit
     - `TransferType::Hello`: logs the peer connection
     - `TransferType::Ping`: debug-level log
   - Ends audit session on disconnect

## Core Changes

### `crates/adtp-core/src/types.rs`
- Added `PEER_ID_SIZE` constant (16 bytes)
- Added `PeerIdentity` struct: `peer_id`, `public_key`, `adtp_version`, `max_file_size`, `supported_mime_types`, `display_name`
- Added `TrustLevel` enum: `Trusted`, `Unverified`, `Blocked`
- Added `PeerEntry` struct: `peer_id`, `public_key`, `nickname`, `host`, `port`, `trust_level`
- Collapsed `Send`/`Push` into single `TransferType::Transfer` variant
- Renamed `Handshake` → `Hello`
- Added `Subscribe`/`Notify` variants (reserved for future use)
- Added `sender_peer_id`/`receiver_peer_id` to `TransferRecord`

### `crates/adtp-core/src/protocol.rs`
- Added `sender_peer_id: [u8; PEER_ID_SIZE]` and `receiver_peer_id: [u8; PEER_ID_SIZE]` to `PacketHeader`
- Updated `PacketHeader::new()` constructor to accept both peer_id fields

### `crates/adtp-core/src/session.rs`
- Added `PersistentPeerIdentity` with `load_or_generate()` method
- Key file persistence to `~/.config/adtp/identity.key`
- `to_peer_identity()` conversion method

## CLI Enhancements

### New `adtp peer` subcommand group
```bash
adtp peer trust --list               # List all known peers
adtp peer trust --add <peer_id>      # Add a peer as trusted
adtp peer trust --remove <peer_id>   # Remove a peer
```

## MCP Tools (Python)

### `send_file.py` and `receive_file.py`
- Added optional `peer_id: str | None` parameter to both `Input` and `Result` models
- Added `_is_valid_peer_id()` validation (32-char hex string)
- Peer ID is validated before bridging to backend

## API Router

### `packages/adtp-api/src/adtp_api/routers/transfers.py`
- Added optional `peer_id: str | None = Query(None)` to `list_transfers`
- Filter matches `sender_peer_id` OR `receiver_peer_id` against the provided peer_id

## Desktop

### `desktop/src/components/PeerCard.js` (NEW)
- Reusable card component for peer management in Settings
- Displays: Peer ID (read-only), Nickname, Host, Port, Trust Level (dropdown)

### `desktop/src/pages/Settings.js`
- Added "Peers" section below "Agents" section
- Renders `PeerCard` list + "Add Peer" button
- Peer entries persisted via `save_settings`/`load_settings` alongside agents

## Testing

### New Integration Tests (`crates/adtp-peer-core/tests/integration.rs`)

| Test | What it verifies |
|---|---|
| `test_identity_generate_and_load` | Key generation, file persistence, peer_id derivation |
| `test_trust_store_add_list_remove` | Add, list, remove peer entries |
| `test_trust_store_persistence` | TOML file round-trip |
| `test_trust_is_allowed` | Trust level gating: Trusted→allowed, Blocked→blocked |
| `test_audit_log_transfer_and_verify` | Full audit cycle: session start, transfer log, verify chain |
| `test_audit_chain_tamper_detection` | Tampering with DB is detected by verify_chain() |

### Test Results
```
cargo test --workspace: 52 passed, 0 failed
  - adtp-core:       14/14
  - adtp-client:      2/2
  - adtp-server:      2/2
  - adtp-cli:        16/16
  - adtp-peer-core:   6/6  (new)
  - adtp-desktop:    11/11
  - adtp-proto:       0/0
  - adtp-peer:        0/0
```

## Security
- **Ed25519 identities**: Each peer generates a unique signing key. Peer ID is a blake3 hash of the public key.
- **Trust gating**: `DualEndpoint` accept loop rejects packets from untrusted senders before any decryption.
- **Tamper-evident audit**: SQLite chain uses blake3 hash chaining. Any modification to logged records is detectable.
- **Key file permissions**: `identity.key` is created with `chmod 600` on Unix.
- **Insecure TLS**: Self-signed certs with `SkipCertVerification` — acceptable for LAN/trusted-network deployments. Future phases will add CA-based verification.

## Build & Run

```bash
# Build all workspace crates
cargo build --workspace

# Run the peer daemon
cargo run -p adtp-peer -- --listen-port 40001

# Run tests
cargo test --workspace

# CLI peer trust management
cargo run -p adtp-cli -- peer trust --list
cargo run -p adtp-cli -- peer trust --add <hex_peer_id>
```

## Phase 8 Handoff Points
- Peer-to-peer trust bootstrapping (out-of-band exchange)
- `Subscribe`/`Notify` implementation for publish-subscribe transfers
- Certificate Authority (CA) for TLS verification instead of `SkipCertVerification`
- Peer discovery (mDNS/LAN broadcast)
- End-to-end encryption with forward secrecy (X25519 + HKDF)
