# ADTP — Phase 13
## CLI Implementation
**Ashir Tech | ashir.world**

---

## Overview

Phase 13 fully realises the `adtp-cli` crate as a complete terminal interface for every ADTP operation — file transfer, agent instruction, peer management, audit, session control, configuration, and REPL — without opening the Desktop App.

Built on the Phase 13 CLI Design Survey, the CLI follows three principles:

- **Fast** — the common operation is always the shortest command. Aliases (`sf`, `recv`, `st`, `pl`, `pa`, `al`, `av`) eliminate repetition.
- **Effective** — every Desktop App GUI operation is available here. Nothing is GUI-only.
- **Nice** — coloured output (Blue Eclipse palette), interactive REPL, live progress bars, embedded agent reachable by plain English.

---

## Entry Point & ASCII Art

When `adtp` runs with no arguments it prints the user-supplied ASCII art header in gold, then drops into the REPL. The art is stored as a static string in `art.rs`.

Status bar below the art:
```
  v0.1.0  |  peer: a3f9bc12  |  -- peers connected  |  agent: --
```

Auto-connect mode attempts daemon connection on launch. If unreachable, segments show as `--`.

---

## Command Structure

```
adtp <verb> [noun] [target] [flags]
```

| Layer | Example | When to use |
|---|---|---|
| Full command | `adtp send file ./report.xlsx --to work-pc` | Scripts, documentation |
| Short alias | `adtp sf ./report.xlsx --to work-pc` | Daily terminal use |
| Natural language | `adtp ask "send the report to work-pc"` | Complex multi-step |

---

## Output Format Flags

| Flag | Output | Use Case |
|---|---|---|
| *(default)* | Coloured table, ANSI codes | Interactive terminal |
| `--format json` | Raw JSON matching Phase 4 API | Scripting, `jq` |
| `--quiet` / `-q` | Suppress all output | Silent scripting |

---

## Commands Implemented

### SEND — `adtp send <file> [--to <peer>] [--wait]`

- Alias: `adtp sf`
- If `--to` is omitted, uses `default_peer` from config or errors
- `--wait` polls every 500ms with live progress bar; shows green tick on completion or red cross on failure
- Without `--wait`, prints transfer ID and exits immediately

### RECEIVE — `adtp receive [--pending] [--delivered] [--failed] [--from <peer>]`

- Alias: `adtp recv`
- Shortcut flags override `--status` if present

### PEER — `adtp peer <subcommand>`

Subcommands:
- `list` — known peers table (aliases: `adtp pl`)
- `add <peer-id> --name <nick>` (alias: `adtp pa`)
- `remove <peer-id-or-nickname>`
- `trust <peer-id>`
- `block <peer-id>`
- `status` — own NAT type, endpoint, rendezvous status
- `scan` — mDNS discovery on local network
- `show <peer-id-or-nickname>` — detailed info

### AGENT — `adtp agent <subcommand>`

- `ask "<instruction>"` — streams response, tool calls printed inline (aliases: `adtp ask`)
- `status` — active state, provider, model, uptime
- `activate` / `deactivate`
- `permissions` — view current rules
- `permissions add-path <path>` / `remove-path <path>` / `allow-command <command>`

### WATCH — `adtp watch <folder> [--to <peer>] [--pattern "*.xlsx"]`

- `adtp watch list` — active watchers
- `adtp watch remove <watch-id>` — stop watching

### AUDIT — `adtp audit <subcommand>`

- `log [--limit] [--from] [--to] [--status] [--peer]` (alias: `adtp al`)
- `verify` — chain integrity check (alias: `adtp av`, exit code 2 for tampered)
- `export [--out <file.csv>]`
- `show <transfer-id>`

### SESSION — `adtp session <subcommand>`

- `list` — active and past sessions
- `show <id>` — detail view
- `close <id>` — close active session
- `transfers <id>` — transfers in a session

### STATUS — `adtp status [<id>] [--watch] [--queue]`

- Alias: `adtp st`
- Without args: box-drawing overview (API, Daemon, Rendezvous, Agent, Queue)
- `--watch` re-renders every 2 seconds
- With transfer ID: detailed progress

### CONFIG — `adtp config <subcommand>`

- `show` — current configuration values
- `set <key> <value>` — update (api-url, api-token, default-peer, etc.)
- `reset` — restore defaults
- `permissions` — view agent permission rules

### HISTORY — `adtp history [--limit <n>]` (alias for `audit log`)
### VERIFY — `adtp verify` (alias for `audit verify`)
### EXPORT — `adtp export [--out <file>]` (alias for `audit export`)
### CONTEXT — `adtp context <id>`
### CONNECT — `adtp connect --host <h> --port <p> --name <n>`

---

## REPL Mode

Running `adtp` with no arguments drops into an interactive REPL:

```
  [ASCII ART]

  v0.1.0  |  peer: a3f9bc12  |  -- peers connected  |  agent: --

  Type a command (without 'adtp') or ask the agent anything.
  'help' for commands · 'exit' or Ctrl+D to quit.

  ▸ _
```

| Feature | Detail |
|---|---|
| Command prefix | Commands entered without `adtp` prefix |
| Agent access | Anything not matching a known command is passed to the agent |
| Tab completion | Completes command names and file paths |
| History | Up/Down arrow cycles through session history |
| Exit | `exit`, `quit`, Ctrl+D or Ctrl+C |

---

## Aliases

| Full Command | Alias |
|---|---|
| `adtp send` | `adtp sf` |
| `adtp receive` | `adtp recv` |
| `adtp status` | `adtp st` |
| `adtp peer list` | `adtp pl` |
| `adtp peer add` | `adtp pa` |
| `adtp audit log` | `adtp al` |
| `adtp audit verify` | `adtp av` |

---

## Error Output & Exit Codes

All errors print to stderr. Exit codes per the spec:

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error |
| 2 | Audit chain tampered |
| 3 | Permission denied |
| 130 | Interrupted by Ctrl+C |

Errors include a `Hint:` line with a human-readable next step.

---

## Colour System — Blue Eclipse Palette

| Element | ANSI Colour |
|---|---|
| ASCII art header | Gold `\x1b[38;2;201;168;76m` |
| Success `✓` | Green `\x1b[38;2;63;166;107m` |
| Failure `✗` | Ember `\x1b[38;2;200;67;10m` |
| In-progress `⟳` | Purple-bright `\x1b[38;2;124;124;191m` |
| Table headers | Muted `\x1b[38;5;244m` |
| Peer IDs, hashes | Purple-bright `\x1b[38;2;124;124;191m` |
| Dim separators | `\x1b[2m` |

Colour suppressed automatically when stdout is not a TTY.

---

## Crate Dependencies

| Crate | Version | Purpose |
|---|---|---|
| clap | 4, derive | Subcommand and flag parsing |
| reqwest | 0.12, rustls-tls | HTTP calls to Phase 4 REST API |
| tokio | workspace | Async runtime |
| serde / serde_json | workspace | Deserialise API responses |
| comfy-table | 7 | Formatted table output |
| indicatif | 0.17 | Progress bars, spinners |
| colored | 2 | ANSI colour output |
| rustyline | 14 | REPL readline — history, tab completion |
| toml | workspace | Config file read/write |
| keyring | 2 | API token from OS keychain |
| supports-color | 3 | TTY colour detection |
| atty | 0.2 | Interactive stdin detection |

---

## File Structure

| File | Purpose |
|---|---|
| `src/main.rs` | Entry point — alias resolution, config load, dispatch or REPL |
| `src/lib.rs` | `Cli`/`Commands` enums, `resolve_aliases()`, `dispatch_command()` |
| `src/art.rs` | ASCII art string, gold colour helpers, status bar builder |
| `src/repl.rs` | REPL mode — rustyline, command dispatch, agent passthrough |
| `src/client.rs` | HTTP client wrapper — GET/POST/DELETE with auth |
| `src/config.rs` | `cli.toml` load/save, env overrides, keychain token storage |
| `src/display.rs` | Blue Eclipse palette, all render functions, progress bars |
| `src/errors.rs` | `CliError` enum (6+ variants), exit codes, hint messages |
| `src/models.rs` | Serde structs for all API response shapes |
| `src/commands/mod.rs` | Module declarations (17 command modules) |
| `src/commands/send.rs` | send + sf |
| `src/commands/receive.rs` | receive + recv |
| `src/commands/peer.rs` | peer list, add, remove, trust, block, status, scan, show |
| `src/commands/agent.rs` | agent ask, status, activate, deactivate, permissions |
| `src/commands/audit.rs` | audit log, verify, export, show |
| `src/commands/config.rs` | config show, set, reset, permissions |
| `src/commands/session.rs` | session list, show, close, transfers |
| `src/commands/status.rs` | status, status --watch, --queue |
| `src/commands/watch.rs` | watch folder registration |
| `src/commands/unwatch.rs` | remove a folder watch |
| `src/commands/history.rs` | history (alias for audit log) |
| `src/commands/verify.rs` | verify (alias for audit verify) |
| `src/commands/export.rs` | export (alias for audit export) |
| `src/commands/context.rs` | transfer context display |
| `src/commands/connect.rs` | agent connection registration |

---

## Tests

16 integration tests using wiremock cover all major command paths:
- Send success, file not found, watch flow
- Receive empty, with results
- Verify valid chain, tampered chain
- Status with daemon down
- Export to file, history with date filter
- Auth invalid token, JSON output format
- Connect writes config, context empty, session list
- Unwatch delete

```
test result: ok. 16 passed; 0 failed
```

---

## Files Changed

| File | Status | Lines |
|---|---|---|
| `Cargo.toml` | Modified | 40 |
| `src/art.rs` | **New** | 38 |
| `src/repl.rs` | **New** | 241 |
| `src/commands/agent.rs` | **New** | 174 |
| `src/commands/audit.rs` | **New** | 188 |
| `src/commands/config.rs` | **New** | 147 |
| `src/commands/peer.rs` | Rewritten | 202 |
| `src/lib.rs` | Rewritten | 134 |
| `src/main.rs` | Rewritten | 102 |
| `src/config.rs` | Updated | 177 |
| `src/display.rs` | Updated | 508 |
| `src/errors.rs` | Updated | 100 |
| `src/models.rs` | Updated | 232 |
| `src/commands/send.rs` | Updated | 140 |
| `src/commands/receive.rs` | Updated | 75 |
| `src/commands/status.rs` | Updated | 116 |
| `src/commands/session.rs` | Updated | 125 |
| `src/commands/connect.rs` | Updated | 65 |
| `tests/integration_test.rs` | Updated | 594 |
| **Total** | | **~3,900** |

---

*Ashir Tech — ashir.world | ADTP Phase 13 CLI Implementation*
