# ADTP Phase 6 — Desktop App (Tauri / Rust + Vanilla JS)

## Overview
Phase 6 delivers a cross-platform desktop GUI for ADTP using Tauri 2. The desktop app gives non-technical users a visual interface for sending files, monitoring transfers, reviewing audit history, and managing settings — all through the Phase 4 REST API.

## Architecture
- **Process Model**: The Desktop App is one of four independent processes (daemon, server, API, desktop). Closing it does not interrupt transfers.
- **Communication**: The Tauri Rust backend (webview ↔ Rust ↔ Phase 4 API) is the single source of truth. The frontend never polls the API directly — it only renders what the backend pushes via Tauri events.
- **Background Polling**: Four `tokio` intervals poll the Phase 4 API (transfers: 2s, health: 5s, queue: 5s, sessions: 10s) and emit Tauri events.

## Package Structure

```
desktop/
├── package.json                    # Frontend deps (@tauri-apps/api, plugin-dialog, plugin-fs, plugin-notification, plugin-store)
├── src/
│   ├── index.html                  # Single-page shell
│   ├── styles.css                  # Dark theme (Ashir brand)
│   ├── app.js                      # Router — mounts Dashboard/History/Settings
│   ├── components/
│   │   ├── DropZone.js             # Drag-drop + click-to-browse file send
│   │   ├── TransferList.js         # Live transfer table (subscribed to transfer-update)
│   │   ├── AgentCard.js            # Agent display/edit inside Settings
│   │   ├── AuditLog.js             # Paginated audit table + verify chain + export CSV
│   │   ├── StatusBar.js            # Connection indicator + queue badge
│   │   └── Toast.js                # In-app toast notifications (success/error/info/warning)
│   └── pages/
│       ├── Dashboard.js            # DropZone + StatusBar + TransferList
│       ├── History.js              # AuditLog + verify + export
│       └── Settings.js             # AgentCard list + masked token field + polling controls
└── src-tauri/
    ├── Cargo.toml                  # tauri 2, plugins, adtp-cli as path dep
    ├── tauri.conf.json             # Window, tray, bundle config
    ├── build.rs                    # Tauri build script
    ├── icons/                      # Tray + app icons (idle/syncing/error states)
    ├── src/
    │   ├── main.rs                 # Entry point → calls lib::run()
    │   ├── lib.rs                  # App builder — plugins, commands, tray, poller, events
    │   ├── commands.rs             # 19 #[tauri::command] handlers wrapping adtp_cli::client
    │   ├── poller.rs               # Background polling + OS notification triggers
    │   ├── settings.rs             # tauri-plugin-store (non-secret) + keyring (API token)
    │   └── tray.rs                 # System tray icon, menu, state transitions
    └── tests/                      # Unit tests (inline in each module)
```

## Key Features
- **Dashboard**: Drag-and-drop or click-to-browse file sending. Live transfer list updates via Tauri events. Status bar shows connection state and queue depth.
- **History**: Filterable audit log. Verify Chain integrity check with visual banner. CSV export with native save dialog.
- **Settings**: API URL, write-only token field (saved to OS keychain via `keyring` crate), agent management, polling interval controls.
- **System Tray**: Three-state icon (idle/syncing/error). Right-click menu: Open, Send File, Status label, Quit. Click to show window.
- **OS Notifications**: Delivered/failed transfer alerts. Connection-loss warnings. Suppressed when window is focused on Dashboard.

## Security
- **API token**: Stored in OS keychain via the `keyring` crate (macOS Keychain, Windows Credential Manager, Linux Secret Service). Never written to disk as plaintext.
- **Non-secret settings**: Stored via `tauri-plugin-store` (plain JSON in app data directory).

## Dependencies
### Rust (Cargo.toml)
- `tauri 2` (features: `tray-icon`)
- `tauri-plugin-dialog 2`, `tauri-plugin-fs 2`, `tauri-plugin-notification 2`, `tauri-plugin-store 2`, `tauri-plugin-shell 2`
- `keyring 3` — OS keychain access
- `adtp-cli` — local path dependency (reuses `config.rs`, `models.rs`, `client.rs`, `errors.rs`)

### Frontend (package.json)
- `@tauri-apps/api ^2` — Tauri IPC `invoke()` and event system
- `@tauri-apps/plugin-dialog ^2`, `@tauri-apps/plugin-fs ^2`, `@tauri-apps/plugin-notification ^2`, `@tauri-apps/plugin-store ^2`

## Phase 5 Modification
One change was made to Phase 5's `crates/adtp-cli/Cargo.toml`: added a `[lib]` target so the desktop crate can import `adtp-cli` as a library dependency. The `[[bin]]` target remains unchanged. A new `src/lib.rs` re-exports all public modules (`commands`, `client`, `config`, `display`, `errors`, `models`).

## Tauri Commands (19 total)
| Command | Method | Endpoint |
|---|---|---|
| `send_file` | POST | `/api/v1/transfers/send` |
| `list_transfers` | GET | `/api/v1/transfers` |
| `get_transfer_status` | GET | `/api/v1/transfers/{id}/status` |
| `get_transfer_context` | GET | `/api/v1/transfers/{id}/context` |
| `cancel_transfer` | DELETE | `/api/v1/transfers/{id}` |
| `register_watch` | POST | `/api/v1/watchers` |
| `remove_watch` | DELETE | `/api/v1/watchers/{id}` |
| `list_watchers` | GET | `/api/v1/watchers` |
| `list_sessions` | GET | `/api/v1/sessions` |
| `get_audit_log` | GET | `/api/v1/audit/log` |
| `verify_chain` | POST | `/api/v1/audit/verify` |
| `export_audit` | GET | `/api/v1/audit/export` |
| `pick_file` | — | Native dialog |
| `pick_save_folder` | — | Native dialog |
| `save_settings` | — | tauri-plugin-store |
| `load_settings` | — | tauri-plugin-store |
| `save_api_token` | — | keyring crate |
| `load_api_token` | — | keyring crate |
| `get_app_health` | — | Cached poller state |

## Polling Events
| Event | Interval | Payload |
|---|---|---|
| `transfer-update` | 2s | `TransferRecord` (for new entries) |
| `health-update` | 5s | `HealthSnapshot` |
| `queue-update` | 5s | `QueueResponse` |
| `session-update` | 10s | `Vec<SessionInfo>` |
| `notify` | — | `{ title, body }` → OS notification or toast |

## Testing
- **Backend**: 11 inline unit tests (4 poller, 2 settings, 3 tray, 2 commands)
- **CLI (untouched)**: 16 integration tests still passing
- **Verification**: `cargo check --workspace` passes with zero warnings. `cargo test -p adtp-desktop` passes 11/11.

## Build
```bash
cd desktop && npm install     # Install frontend deps
cargo tauri build             # Build unsigned installer bundles
cargo tauri dev               # Run in development mode
```

## Phase 7 Handoff Points
- Unsigned `.app` / `.exe` / `.AppImage` bundles from `cargo tauri build`
- Auto-update disabled in `tauri.conf.json`
- No crash reporting / telemetry
- Version `0.1.0` in `Cargo.toml`
