# ADTP Phase 5 — CLI (`adtp`)

## Build Spec Reference

`phases/ADTP_Phase5_Build_Spec.docx`

---

## Summary

Phase 5 produces the `adtp` binary — a single compiled Rust executable with zero Python dependencies. It communicates exclusively with the Phase 4 REST API over HTTP to `localhost:40003`. It does not call the Rust daemons, the MCP server, or the SQLite database directly. Every command follows the same pattern: parse args → load config → call API → render output → exit.

---

## Files (18 + 1 test file)

### Crate: `crates/adtp-cli/`

| Path | Lines | Purpose |
|------|-------|---------|
| `Cargo.toml` | 17 | Package metadata + deps (clap, reqwest, tokio, serde, comfy-table, indicatif, colored, toml, dirs) |
| `src/main.rs` | 91 | Entry point — clap arg parsing, dispatch to subcommands, tracing init |
| `src/config.rs` | 99 | `CliConfig` — load/save `~/.config/adtp/cli.toml`, env var overrides |
| `src/client.rs` | 170 | `ApiClient` — reqwest wrapper with token injection, GET/POST/DELETE, error mapping |
| `src/display.rs` | 165 | Terminal rendering — comfy-table tables, coloured status badges, progress bars, context display |
| `src/errors.rs` | 63 | `CliError` enum — maps HTTP status to user-friendly messages + exit codes |
| `src/models.rs` | 148 | Serde structs mirroring Phase 4 API response shapes (16 structs) |
| `src/commands/mod.rs` | 11 | Re-exports all 11 command modules |
| `src/commands/send.rs` | 87 | `adtp send` — POST transfer, optional `--wait` with progress bar |
| `src/commands/receive.rs` | 50 | `adtp receive` — GET transfers list, table render |
| `src/commands/watch.rs` | 43 | `adtp watch` — POST watcher registration |
| `src/commands/unwatch.rs` | 27 | `adtp unwatch` — DELETE watcher |
| `src/commands/status.rs` | 76 | `adtp status` — daemon health + queue depth, single transfer tracking with `--watch` |
| `src/commands/history.rs` | 61 | `adtp history` — audit log query with date/session/status filters |
| `src/commands/verify.rs` | 44 | `adtp verify` — chain integrity verification |
| `src/commands/connect.rs` | 54 | `adtp connect` — writes agent entry to `cli.toml` |
| `src/commands/context.rs` | 24 | `adtp context` — GET transfer context block |
| `src/commands/export.rs` | 53 | `adtp export` — GET audit export CSV, write to file or stdout |
| `src/commands/session.rs` | 55 | `adtp session` — list sessions or get session detail with transfers |
| `tests/integration_test.rs` | 590 | Wiremock integration tests (16 tests covering all commands) |

### Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| clap | 4 (derive) | CLI argument parsing |
| reqwest | 0.12 (json + rustls-tls) | Async HTTP client |
| tokio | workspace (full) | Async runtime |
| serde | workspace + derive | Serialisation for API req/resp |
| serde_json | workspace | JSON parsing and pretty-print |
| comfy-table | 7 | Terminal table rendering |
| indicatif | 0.17 | Progress bars and spinners |
| colored | 2 | Coloured terminal output |
| toml | workspace | Config file read/write |
| dirs | workspace | `~/.config/adtp/` directory |
| chrono | workspace | Timestamp formatting |
| uuid | workspace | Transfer ID display |
| wiremock | 0.6 (dev) | Mock HTTP server for tests |

---

## CLI Commands

| Command | Usage | API Call |
|---------|-------|----------|
| `send` | `adtp send <file> [--agent] [--priority] [--message] [--wait]` | `POST /api/v1/transfers/send` + poll status |
| `receive` | `adtp receive [--status] [--limit] [--session]` | `GET /api/v1/transfers` |
| `watch` | `adtp watch <folder> [--pattern] [--agent] [--action]` | `POST /api/v1/watchers` |
| `unwatch` | `adtp unwatch <id>` | `DELETE /api/v1/watchers/{id}` |
| `status` | `adtp status [id] [--watch]` | `GET /health/ready` + `/health/queue` or `/transfers/{id}/status` |
| `history` | `adtp history [--limit] [--status] [--from] [--to] [--session]` | `GET /api/v1/audit/log` |
| `verify` | `adtp verify` | `POST /api/v1/audit/verify` |
| `connect` | `adtp connect --host --port --name [--default]` | No API call (writes config) |
| `context` | `adtp context <id>` | `GET /api/v1/transfers/{id}/context` |
| `export` | `adtp export [--output] [--from] [--to] [--session]` | `GET /api/v1/audit/export` |
| `session` | `adtp session [id]` | `GET /api/v1/sessions` or `/sessions/{id}` |

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (API error, auth failure, file not found, daemon unreachable, rate limited) |
| 2 | Chain tampered or file write permission error |

---

## Configuration

Config file: `~/.config/adtp/cli.toml`. All fields overridable via environment variables.

| TOML Field | Env Var | Default | Description |
|------------|---------|---------|-------------|
| `api_url` | `ADTP_API_URL` | `http://127.0.0.1:40003` | Phase 4 API base URL |
| `api_token` | `ADTP_API_TOKEN` | — | Bearer token (required) |
| `default_agent` | `ADTP_DEFAULT_AGENT` | `default` | Agent name for send |
| `output_format` | `ADTP_OUTPUT_FORMAT` | `table` | `table`, `json`, or `plain` |
| `poll_interval_ms` | `ADTP_POLL_INTERVAL` | `500` | Status polling interval |
| `connect_timeout_ms` | `ADTP_CONNECT_TIMEOUT` | `5000` | HTTP connect timeout |
| `request_timeout_ms` | `ADTP_REQUEST_TIMEOUT` | `30000` | HTTP request timeout |

---

## Output Formats

All commands support `--format` flag or `output_format` in config:

- **table** (default): comfy-table with borders, coloured status, aligned columns
- **json**: Raw JSON array/object matching API response — no formatting, no colour
- **plain**: Key=value format — no ANSI codes, safe for log ingestion

---

## How to Build

```bash
# Build release binary
cargo build --release --package adtp-cli

# Binary is at target/release/adtp (5.7MB stripped)
strip target/release/adtp  # already stripped by workspace profile
```

## How to Test

```bash
# Run all tests
cargo test --package adtp-cli

# Run specific test
cargo test --package adtp-cli test_send_success
```

## How to Use

```bash
# Set token (or write ~/.config/adtp/cli.toml)
export ADTP_API_TOKEN="your-token-here"

# List transfers
./target/release/adtp receive

# Send a file
./target/release/adtp send /path/to/file --wait

# Check daemon status
./target/release/adtp status
```

---

## Test Results

```
test result: ok. 16 passed; 0 failed
```

| Test | Description |
|------|-------------|
| `test_send_success` | POST /send returns transfer_id |
| `test_send_file_not_found` | 404 returns error message |
| `test_receive_empty` | Empty list returns no-transfers message |
| `test_receive_with_results` | 3 records render in table |
| `test_verify_valid_chain` | VALID output, exit 0 |
| `test_verify_tampered_chain` | TAMPERED output, exit 2 |
| `test_status_daemon_down` | 503 displays unreachable |
| `test_export_to_file` | CSV written to temp file |
| `test_auth_invalid_token` | 401 displays auth error |
| `test_json_output_format` | Output is valid JSON |
| `test_connect_writes_config` | cli.toml written and verified |
| `test_context_empty` | Null context renders correctly |
| `test_session_list` | Session table renders |
| `test_history_with_date_filter` | Query params sent correctly |
| `test_send_watch_flow` | POST watcher returns watch_id |
| `test_unwatch_delete` | DELETE returns success |

---

## Phase 6 Hook Points

| Shared Module | Used By Phase 6 | How |
|---|---|---|
| `src/config.rs` | Tauri backend | Reads same `cli.toml` for API URL and token |
| `src/models.rs` | Tauri backend | Same serde structs deserialise API responses |
| `src/errors.rs` | Tauri backend | Same error types serialised to JSON for frontend |
| `src/client.rs` | Tauri backend | HTTP client wrapper reused in Tauri commands |

Every Phase 6 UI action maps to an endpoint already defined in Phase 4 and already called by the Phase 5 CLI. Phase 6 is purely a rendering layer on top of the existing API surface.

---

*Phase 5 — Completed 2026-06-15*
