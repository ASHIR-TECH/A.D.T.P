# ADTP Phase 4 — REST API

## Build Spec Reference

`phases/ADTP_Phase4_Build_Spec.docx`

---

## Summary

Phase 4 exposes the ADTP transfer system as a FastAPI REST API. It reads from the same SQLite audit DB used by Phase 2's `adtp-client` daemon and calls Phase 2 bridge functions via direct Python imports. It provides endpoints for transfers, sessions, audit logs, folder watchers, and health checks. Authentication is HMAC Bearer token (≥ 32 chars), rate limiting is per-IP via slowapi (default 100/min), and CORS defaults to `http://localhost:*`.

---

## Files (21 total)

### New Files (all 21)

| Path | Lines | Purpose |
|------|-------|---------|
| `pyproject.toml` | 37 | Project metadata, deps, hatchling build config |
| `src/adtp_api/__init__.py` | 0 | Package marker |
| `src/adtp_api/config.py` | 25 | `Settings` via `pydantic-settings` (prefix `ADTP_`) |
| `src/adtp_api/dependencies.py` | 17 | `get_db` — aiosqlite connection generator |
| `src/adtp_api/errors.py` | 56 | Custom exceptions + FastAPI error handler registration |
| `src/adtp_api/main.py` | 72 | App factory `create_app()` + `main()` uvicorn entrypoint |
| `src/adtp_api/models/__init__.py` | 0 | Models package marker |
| `src/adtp_api/models/audit.py` | 36 | `AuditEntry`, `AuditLogList`, `ChainVerifyResult` |
| `src/adtp_api/models/context.py` | 17 | `ContextBlock`, `TransferHistoryEntry` |
| `src/adtp_api/models/session.py` | 17 | `SessionInfo`, `SessionList` |
| `src/adtp_api/models/transfer.py` | 51 | `TransferRequest`, `TransferResponse`, `TransferRecord`, `TransferStatus`, `TransferList` |
| `src/adtp_api/models/watcher.py` | 24 | `WatchRequest`, `WatchResponse`, `WatchList` |
| `src/adtp_api/middleware/auth.py` | 34 | HMAC Bearer token auth middleware |
| `src/adtp_api/middleware/ratelimit.py` | 18 | slowapi rate-limit registration |
| `src/adtp_api/routers/__init__.py` | 0 | Routers package marker |
| `src/adtp_api/routers/health.py` | 46 | `GET /health`, `/health/ready`, `/health/queue` |
| `src/adtp_api/routers/transfers.py` | 62 | Send, push, list, get, cancel, status, context |
| `src/adtp_api/routers/sessions.py` | 33 | List, get, transfers, close |
| `src/adtp_api/routers/watchers.py` | 45 | Register, list, get, remove |
| `src/adtp_api/routers/audit.py` | 48 | Query log, get entry, export CSV, verify chain |
| `src/adtp_api/services/transfer_service.py` | 115 | Bridge calls, DB queries for transfers |
| `src/adtp_api/services/session_service.py` | 68 | Session info from audit DB |
| `src/adtp_api/services/audit_service.py` | 148 | Audit log query, CSV export, chain verification |

---

## API Endpoints

### Health (no auth)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/health` | Basic health (status, version, uptime) |
| `GET` | `/api/v1/health/ready` | Readiness (daemon connected?, audit DB exists?) |
| `GET` | `/api/v1/health/queue` | Queue depth via bridge |

### Transfers

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/api/v1/transfers/send` | Bearer | Enqueue file for send (rate-limited: 10/min) |
| `POST` | `/api/v1/transfers/push` | Bearer | Push file to server |
| `GET` | `/api/v1/transfers` | Bearer | List transfers (filter by status, session) |
| `GET` | `/api/v1/transfers/{id}` | Bearer | Get transfer record |
| `DELETE` | `/api/v1/transfers/{id}` | Bearer | Cancel pending transfer |
| `GET` | `/api/v1/transfers/{id}/status` | Bearer | Real-time transfer status |
| `GET` | `/api/v1/transfers/{id}/context` | Bearer | Get Phase 3 context block |

### Sessions

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/v1/sessions` | Bearer | List all sessions |
| `GET` | `/api/v1/sessions/{id}` | Bearer | Get session info |
| `GET` | `/api/v1/sessions/{id}/transfers` | Bearer | List transfers in session |
| `DELETE` | `/api/v1/sessions/{id}` | Bearer | Close session |

### Watchers

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/api/v1/watchers` | Bearer | Register folder watch |
| `GET` | `/api/v1/watchers` | Bearer | List watches |
| `GET` | `/api/v1/watchers/{id}` | Bearer | Get watch |
| `DELETE` | `/api/v1/watchers/{id}` | Bearer | Remove watch |

### Audit

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/v1/audit/log` | Bearer | Query audit log (filter by session, type, status, time range) |
| `GET` | `/api/v1/audit/log/{id}` | Bearer | Get single audit entry |
| `GET` | `/api/v1/audit/export` | Bearer | Export audit log as CSV |
| `POST` | `/api/v1/audit/verify` | Bearer | Verify chain integrity (blake3 hashing) |

---

## Configuration

| Env Variable | Default | Description |
|---|---|---|
| `ADTP_API_PORT` | `40003` | Server port |
| `ADTP_API_HOST` | `127.0.0.1` | Bind address. Set `0.0.0.0` to expose the gateway on the LAN (required for QR mobile pairing) |
| `ADTP_API_TOKEN` | _(required)_ | Bearer token (≥ 32 chars) |
| `ADTP_CLIENT_SOCKET` | `/tmp/adtp-client.sock` | Phase 2 client socket path |
| `ADTP_SERVER_SOCKET` | `/tmp/adtp-server.sock` | Phase 2 server socket path |
| `ADTP_AUDIT_DB` | `~/.config/adtp/client-audit.db` | SQLite audit DB path |
| `ADTP_CORS_ORIGINS` | `http://localhost:*` | Comma-separated CORS origins |
| `ADTP_RATE_LIMIT` | `100/minute` | Default rate limit |
| `ADTP_API_DOCS` | `true` | Enable `/docs` and `/redoc` |
| `ADTP_LOG_LEVEL` | `INFO` | Logging level |
| `ADTP_MAX_FILE_SIZE` | `10GB` | Max accepted file size |

---

## Authentication

- All endpoints require `Authorization: Bearer <token>` header except `/api/v1/health/*`, `/docs`, `/redoc`, `/openapi.json`.
- Token validated at startup: must be ≥ 32 characters.
- Comparison uses `hmac.compare_digest` (constant-time).

## Rate Limiting

- Default: 100 requests/minute per IP (configurable via `ADTP_RATE_LIMIT`).
- Send file: 10 requests/minute (hardcoded).
- Implemented via slowapi with `get_remote_address` key function.

## Error Responses

| Status | Meaning |
|--------|---------|
| 401 | Missing/invalid Bearer token |
| 404 | Transfer, session, watch, or file not found |
| 409 | Transfer already delivered (can't cancel) |
| 413 | File exceeds max size |
| 429 | Rate limit exceeded |
| 503 | Daemon unreachable or not ready |

---

## Architecture

```
HTTP Client
  │
  ▼
FastAPI (Uvicorn)
  │
  ├── Auth Middleware (HMAC Bearer)
  ├── Rate Limit Middleware (slowapi)
  ├── CORS Middleware
  │
  ├── Routers → Services → aiosqlite (audit DB)
  │                     │
  │                     └── adtp_mcp.bridge.* (Phase 2 daemon calls)
  │
  └── Error Handlers
```

---

## How to Run

```bash
# Set required token
export ADTP_API_TOKEN="your-32-char-minimum-token-here"

# Bind to all interfaces so the MARS mobile companion (which reads the QR
# pairing payload) can reach the gateway over the LAN. Keep 127.0.0.1 for
# loopback-only setups.
export ADTP_API_HOST="0.0.0.0"

# Run with uvicorn (from package root)
uv run --directory packages/adtp-api adtp-api

# Or directly
ADTP_API_TOKEN="..." python -m adtp_api.main
```

## How to Test

```bash
source .venv/bin/activate
PYTHONPATH=src pytest packages/adtp-api/tests/ -v
```

---

## Phase 4 Hook Points (for Phase 5)

| Hook | Location | Phase 5 Consumption |
|---|---|---|
| Context JSON | `GET /api/v1/transfers/{id}/context` | Agent decision pipeline |
| Audit export | `GET /api/v1/audit/export` | External SIEM integration |
| Chain verify | `POST /api/v1/audit/verify` | Tamper detection alerts |
| Rate limit headers | `X-RateLimit-*` on every response | Client backoff logic |
| Health/readiness | `GET /api/v1/health/ready` | K8s liveness probes |
| Watcher CRUD | `POST/DELETE /api/v1/watchers` | Automated folder monitoring |
| Session transfers | `GET /api/v1/sessions/{id}/transfers` | Session-level analytics |

---

*Phase 4 — Completed 2026-06-15*
