# Linux Server Deployment

Deploy `adtp-server` on the AI agent host.

## Steps

1. Install Rust.
2. Build the workspace.
3. Create a server config.
4. Open UDP port `40001`.
5. Run `cargo run --package adtp-server` during development.
6. Replace development TLS before production.

```mermaid
flowchart TD
    Build[Build Binary] --> Config[Write server.toml]
    Config --> Firewall[Open UDP 40001]
    Firewall --> Run[Start adtp-server]
    Run --> Logs[Monitor logs and audit DB]
```

## Validation Commands

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo audit
```

## Runtime Ports

| Service | Protocol | Default |
| --- | --- | --- |
| `adtp-server` | QUIC/UDP | `40001` |

## Files and Directories

| Item | Default |
| --- | --- |
| Client config | `~/.config/adtp/client.toml` |
| Server config | OS config dir + `adtp/server.toml` |
| Client receive dir | `~/adtp-received` |
| Server intake dir | OS data dir + `adtp/intake` |
| Server output dir | OS data dir + `adtp/output` |
