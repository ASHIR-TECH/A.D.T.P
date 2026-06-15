from __future__ import annotations

import time
from pathlib import Path

from fastapi import APIRouter

from adtp_api.config import settings

router = APIRouter(prefix="/api/v1/health", tags=["health"])
_start_time = time.time()


@router.get("")
async def health():
    return {
        "status": "ok",
        "version": "0.1.0",
        "uptime_seconds": int(time.time() - _start_time),
    }


@router.get("/ready")
async def readiness():
    daemon_connected = Path(settings.client_socket).exists()
    audit_db_ok = Path(settings.audit_db).exists()
    bridge_ok = daemon_connected
    ready = daemon_connected and bridge_ok
    status_code = 200 if ready else 503
    from fastapi.responses import JSONResponse

    return JSONResponse(
        content={
            "ready": ready,
            "daemon_connected": daemon_connected,
            "audit_db_ok": audit_db_ok,
            "bridge_ok": bridge_ok,
        },
        status_code=status_code,
    )


@router.get("/queue")
async def queue_depth():
    from adtp_mcp.bridge import client as bridge_client

    try:
        result = await bridge_client.pending_files()
        pending = len(result.get("files", [])) if isinstance(result, dict) else 0
    except Exception:
        pending = -1
    return {
        "pending": pending,
        "in_progress": 0,
        "failed_last_hour": 0,
        "avg_delivery_ms": 0,
    }
