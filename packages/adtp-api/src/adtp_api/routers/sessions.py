from __future__ import annotations

from fastapi import APIRouter, Depends

import aiosqlite

from adtp_api.dependencies import get_db
from adtp_api.models.session import SessionInfo, SessionList
from adtp_api.models.transfer import TransferList
from adtp_api.services import session_service

router = APIRouter(prefix="/api/v1/sessions", tags=["sessions"])


@router.get("")
async def list_sessions(db: aiosqlite.Connection = Depends(get_db)) -> SessionList:
    return await session_service.list_sessions(db)


@router.get("/{session_id}")
async def get_session(
    session_id: str, db: aiosqlite.Connection = Depends(get_db)
) -> SessionInfo:
    return await session_service.get_session(db, session_id)


@router.get("/{session_id}/transfers")
async def get_session_transfers(
    session_id: str, db: aiosqlite.Connection = Depends(get_db)
) -> TransferList:
    return await session_service.get_session_transfers(db, session_id)


@router.delete("/{session_id}")
async def close_session(session_id: str):
    result = await session_service.close_session(session_id)
    return {"closed": result}
