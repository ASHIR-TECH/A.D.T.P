# Changelog

All notable changes to ADTP are documented here.

## Unreleased

### Added

- Phase 0 `adtp-core` foundation:
  - Shared protocol types and constants.
  - Typed error model.
  - ADTP packet serialization/deserialization.
  - BLAKE3 hashing and audit-chain helpers.
  - AES-256-GCM payload encryption/decryption.
  - Session key scaffolding.
  - QUIC client/server transport helpers.
- Phase 1 client/server daemons:
  - Client config loading/saving.
  - Server config loading/saving.
  - Client transfer queue.
  - Client push receiver.
  - Client folder watcher.
  - Server intake validation.
  - Server manifest dispatcher.
  - Server push handler.
  - SQLite audit logs with hash chains.
  - Integration tests for QUIC transport, audit chain, intake, and dispatch.

### Changed

- `Cargo.lock` is tracked because the workspace now includes runnable binary crates and `cargo audit` depends on lockfile state.

### Security

- Added explicit audit-chain verification tests.
- Documented Phase 1 cryptographic limitations and production hardening requirements.

## Phase A — Critical Security Fixes

### Fixed

- **A1: TLS Certificate Verification (TOFU)**
  - Replaced `SkipCertVerification` (which accepted ANY TLS certificate) with Trust On First Use (TOFU) model.
  - Client now verifies server certificates against a persistent trust store at `~/.config/adtp/trusted_certs.toml`.
  - First contact: certificate is accepted, SHA-256 fingerprint stored.
  - Subsequent contacts: fingerprint is verified; mismatch triggers hard error with MITM warning.
  - TLS handshake signatures (TLS 1.2/1.3) are now properly verified using rustls crypto provider.
  - Files changed: `crates/adtp-core/src/tofu.rs` (new), `crates/adtp-core/src/transfer.rs`, `crates/adtp-peer-core/src/dual_endpoint.rs`.

- **A2: Rendezvous Server Authentication**
  - Token verification now uses HMAC-SHA256 instead of just checking a consumed set. Tokens are cryptographically bound to requester, target, and timestamp.
  - Token expiry is now enforced (TOKEN_VALIDITY_SECS = 30s).
  - Peer registration now supports Ed25519 signature verification: peers can sign `(peer_id || external_addr || timestamp)` and the server verifies against a registered public key.
  - Files changed: `crates/adtp-rendezvous/src/token.rs`, `crates/adtp-rendezvous/src/peer_registry.rs` (new), `crates/adtp-rendezvous/src/main.rs`, `crates/adtp-core/src/types.rs`, `crates/adtp-peer-core/src/rendezvous_client.rs`, `crates/adtp-peer-core/src/connect.rs`.

- **A3: Server Session Key (ECDH Key Exchange)**
  - Replaced hardcoded `[0u8; 32]` session key with X25519 ephemeral ECDH key exchange.
  - Both client and server generate ephemeral key pairs and exchange public keys in the first QUIC stream.
  - Shared AES-256 session key is derived via HKDF-SHA256 from the ECDH shared secret.
  - Files changed: `crates/adtp-core/src/session.rs`, `crates/adtp-core/src/transfer.rs`, `crates/adtp-client/src/daemon.rs`, `crates/adtp-server/src/daemon.rs`.

- **A4: `shell_exec` Permission Bypass**
  - `shell_exec` handler now uses `PermissionStore::check_command()` instead of directly checking `allowed_commands`, properly enforcing `BLOCKED_COMMANDS` (rm, sudo, kill, etc.).
  - Added command injection sanitization: rejects commands containing `;`, `&&`, `||`, `|`, backticks, `$()`, absolute paths, and directory traversal.
  - Files changed: `crates/adtp-peer-core/src/agent/runtime.rs`, `crates/adtp-peer-core/src/agent/permissions.rs`.

- **A5: File Access Bypass (Symlink Escape)**
  - Replaced all `starts_with()` string prefix checks with `PermissionStore::check_read_path()` which uses `canonicalize()` for symlink-safe path resolution.
  - `find_file` now defaults to first `allowed_paths` entry instead of `"."` when no directory specified.
  - Added `open_bi()` and `accept_bi()` methods to `AdtpConnection` for ECDH handshake.
  - Files changed: `crates/adtp-peer-core/src/agent/runtime.rs`, `crates/adtp-core/src/transfer.rs`.

### Changed

- `.gitignore`: Removed duplicate `target/` entry, added `node_modules/`, `temp/`, `.venv/`, `__pycache__/`, `dist/`.
- Removed tracked `temp/error.png` and `temp/orb-background.html` from git.
- Created `.env.example` template with placeholder values.
- Added `dirs`, `toml`, `hmac`, `sha2`, `ed25519-dalek` dependencies to affected crates.
