# Air-Gapped Deployment

Air-gapped deployment is planned for hardened environments.

## Requirements

- Vendored Rust dependencies or internal registry mirror.
- Offline advisory database for `cargo audit` where possible.
- Pre-provisioned TLS certificates.
- Explicit file intake/output directories.

```mermaid
flowchart TD
    BuildHost[Connected Build Host] --> Vendor[Vendor Dependencies]
    Vendor --> Offline[Air-Gapped Network]
    Offline --> Server[adtp-server]
    Offline --> Client[adtp-client]
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
