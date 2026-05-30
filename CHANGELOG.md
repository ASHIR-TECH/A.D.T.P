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
