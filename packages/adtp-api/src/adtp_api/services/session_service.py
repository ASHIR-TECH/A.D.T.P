from __future__ import annotations

import aiosqlite

from adtp_api.errors import SessionNotFoundError
from adtp_api.models.session import SessionInfo, SessionList
from adtp_api.models.transfer import TransferList, TransferRecord


async def list_sessions(db: aiosqlite.Connection) -> SessionList:
    rows = await db.execute_fetchall(
        "SELECT DISTINCT session_id FROM transfer_audit ORDER BY session_id"
    )
    sessions = []
    for r in rows:
        sid = str(r[0])
        info = await _build_session_info(db, sid)
        sessions.append(info)
    return SessionList(sessions=sessions, total=len(sessions))


async def get_session(db: aiosqlite.Connection, session_id: str) -> SessionInfo:
    rows = await db.execute_fetchall(
        "SELECT * FROM transfer_audit WHERE session_id = ? LIMIT 1", [session_id]
    )
    if not rows:
        raise SessionNotFoundError(session_id)
    return await _build_session_info(db, session_id)


async def get_session_transfers(
    db: aiosqlite.Connection, session_id: str
) -> TransferList:
    rows = await db.execute_fetchall(
        "SELECT * FROM transfer_audit WHERE session_id = ? ORDER BY initiated_at DESC",
        [session_id],
    )
    transfers = [
        TransferRecord(
            id=str(r["id"]) if "id" in r.keys() else "",
            session_id=str(r["session_id"]) if "session_id" in r.keys() else "",
            transfer_type=str(
                r["transfer_type"] if "transfer_type" in r.keys() else "send"
            ),
            filename=str(r["filename"] if "filename" in r.keys() else ""),
            mime_type=str(r["mime_type"] if "mime_type" in r.keys() else ""),
            file_size=int(r["file_size"]) if "file_size" in r.keys() else 0,
            status=str(r["status"] if "status" in r.keys() else "pending"),
            integrity_verified=bool(r["integrity_verified"])
            if "integrity_verified" in r.keys()
            else False,
            signature=str(r["signature"] if "signature" in r.keys() else ""),
            chain_hash=str(r["chain_hash"] if "chain_hash" in r.keys() else ""),
            initiated_at=str(r["initiated_at"] if "initiated_at" in r.keys() else ""),
            completed_at=str(r["completed_at"])
            if "completed_at" in r.keys() and r["completed_at"]
            else None,
        )
        for r in rows
    ]
    return TransferList(
        transfers=transfers, total=len(transfers), limit=len(transfers), offset=0
    )


async def close_session(session_id: str) -> bool:
    from adtp_mcp.bridge import client as bridge_client

    return await bridge_client.close_session(session_id)


async def _build_session_info(db: aiosqlite.Connection, session_id: str) -> SessionInfo:
    rows = await db.execute_fetchall(
        "SELECT * FROM transfer_audit WHERE session_id = ? ORDER BY initiated_at ASC",
        [session_id],
    )
    transfer_count = len(rows)
    bytes_transferred = sum(
        int(r["file_size"]) for r in rows if "file_size" in r.keys()
    )
    started_at = str(rows[0]["initiated_at"]) if rows else ""
    ended_at = (
        str(rows[-1]["completed_at"])
        if rows and "completed_at" in rows[-1].keys() and rows[-1]["completed_at"]
        else None
    )

    return SessionInfo(
        id=session_id,
        started_at=started_at,
        ended_at=ended_at,
        transfer_count=transfer_count,
        bytes_transferred=bytes_transferred,
    )
