# ADTP ↔ MARS blob media alignment

Date: 26 November 2026. Scope: ADTP desktop, FastAPI gateway, Rust core/CLI, compared against the MARS v2 contract implemented on `Blob-feature` (see `docs/BLOB_FEATURE_STATUS_MARS.md` in the MARS repo).

## Contract parity

| Field / rule | MARS | ADTP (after this work) |
|---|---|---|
| `schemaVersion` | v1 when absent, v2 explicit | v1 when absent, v2 explicit (Python, Rust) |
| `storedSize` with `compressedSize` alias | preferred via `storedSizeOf()` | preferred in gateway, UI helper and Rust `stored_size` |
| `expiresAt` / `status` | v2 lifecycle state, expired/deleted detection | same state machine in Pydantic, Rust and both chat UIs |
| id / name validation | non-empty, ≤255, no separators/control chars | same in Python and Rust validators |
| MIME | contains `/`, ≤150 chars | same |
| `sha256` | 32–128 hex chars | same |
| host allowlist | optional `EXPO_PUBLIC_MEDIA_HOSTS` | optional `ADTP_MEDIA_ALLOWED_HOSTS` (supports subdomains) |
| compression | longest edge 1600 / thumb 320, PNG alpha and animated GIF/WebP preserved | identical policy in `media_service.compress_image` |
| recipient / conversation binding | carried in MediaRef | uploaded via multipart `recipient` / `conversationId`, stored on relay record |

## Changes made

- `models/media.py`: v1 upgrade always yields schemaVersion 1; bounded/sanitised id and name; MIME and checksum validation.
- `crates/adtp-core/src/media/validation.rs`: matching id/name/MIME/checksum rules.
- `crates/adtp-cli/src/client.rs`, Tauri command and desktop chat components: optional `sessionId`, `conversationId`, `recipient` are now sent with uploads.
- Gateway `/api/v1/media/upload`: forwards the v2 binding fields to the relay.
- New `desktop/src/lib/mediaCard.js`: shared renderer rules — use `storedSize`, hide thumbnails and disable downloads for expired/deleted media, label states identically to MARS `MediaCard`; `rel="noopener noreferrer"` on external downloads.

## Verification

- ADTP gateway pytest: **66 passed**
- ADTP MCP pytest: **72 passed**
- `cargo test -p adtp-core`: **24 passed**
- `cargo fmt --all --check`: clean; all desktop JS files pass `node --check`
- Live v2 alignment E2E: **17/17** — gateway upload emits full v2 MediaRef, relay metadata matches, media-only device message keeps the ref, header-free thumbnail read works.

MARS gates (on the parallel `Blob-feature` branch): `tsc` clean, **260 Jest tests / 45 suites**.
