from __future__ import annotations

from fastapi import APIRouter, Depends, Query
from fastapi.responses import PlainTextResponse

import aiosqlite

from adtp_api.dependencies import get_db
from adtp_api.models.audit import AuditLogList, ChainVerifyResult
from adtp_api.services import audit_service

router = APIRouter(prefix="/api/v1/audit", tags=["audit"])


@router.get("/log")
async def query_log(
    session_id: str | None = Query(None),
    transfer_type: str | None = Query(None, pattern="^(send|push)$"),
    status: str | None = Query(None),
    from_ts: str | None = Query(None),
    to_ts: str | None = Query(None),
    limit: int = Query(20, ge=1, le=100),
    offset: int = Query(0, ge=0),
    db: aiosqlite.Connection = Depends(get_db),
) -> AuditLogList:
    return await audit_service.query_log(
        db, session_id, transfer_type, status, from_ts, to_ts, limit, offset
    )


@router.get("/log/{transfer_id}")
async def get_audit_entry(transfer_id: str, db: aiosqlite.Connection = Depends(get_db)):
    return await audit_service.get_entry(db, transfer_id)


@router.get("/export")
async def export_audit_csv(
    session_id: str | None = Query(None),
    from_ts: str | None = Query(None),
    to_ts: str | None = Query(None),
    db: aiosqlite.Connection = Depends(get_db),
):
    csv_content = await audit_service.export_csv(db, session_id, from_ts, to_ts)
    return PlainTextResponse(
        content=csv_content,
        media_type="text/csv",
        headers={"Content-Disposition": "attachment; filename=adtp-audit.csv"},
    )


@router.post("/verify")
async def verify_chain(db: aiosqlite.Connection = Depends(get_db)) -> ChainVerifyResult:
    return await audit_service.verify_chain(db)
