from __future__ import annotations

import hashlib
import json
import uuid
from datetime import datetime, timezone
from pathlib import Path

import aiosqlite

from adtp_api.errors import FileNotFoundError_, FileTooLargeError, TransferNotFoundError
from adtp_api.models.transfer import TransferRecord, TransferList, TransferStatus
from adtp_api.config import settings
from adtp_api.services.system_service import resolved_intake_dir


async def send_file(
    file_path: str,
    agent_name: str = "default",
    priority: str = "normal",
    message: str | None = None,
) -> dict:
    from adtp_mcp.bridge import client as bridge_client

    path = Path(file_path)
    if not path.exists():
        raise FileNotFoundError_(file_path)
    if path.stat().st_size > settings.max_file_size:
        raise FileTooLargeError(settings.max_file_size)

    return await bridge_client.enqueue_send(file_path, agent_name, priority)


async def push_file(
    file_path: str, destination: str | None = None, message: str | None = None
) -> dict:
    from adtp_mcp.bridge import server as bridge_server

    return await bridge_server.push_file(file_path, destination)


def _safe_filename(name: str | None) -> str:
    """Strip any client-supplied path components (never trust the phone)."""
    cleaned = Path(name or "").name.strip()
    if not cleaned or cleaned in {".", ".."}:
        return "upload.bin"
    return cleaned


def _unique_destination(directory: Path, filename: str) -> Path:
    """Avoid clobbering an existing intake file by suffixing `-1`, `-2`, …"""
    candidate = directory / filename
    if not candidate.exists():
        return candidate
    stem, suffix = candidate.stem, candidate.suffix
    for index in range(1, 10_000):
        alternate = directory / f"{stem}-{index}{suffix}"
        if not alternate.exists():
            return alternate
    return directory / f"{uuid.uuid4().hex}-{filename}"


async def receive_upload(upload) -> dict:
    """Stream a multipart upload into the intake dir and hash it.

    The phone sends the file bytes; the desktop agent then works with the
    returned absolute `path` (read_file/list_directory/send_file), which is why
    we persist to disk rather than forwarding anywhere.
    """
    intake = resolved_intake_dir()
    intake.mkdir(parents=True, exist_ok=True)

    destination = _unique_destination(intake, _safe_filename(upload.filename))
    hasher = hashlib.sha256()
    size = 0
    try:
        with destination.open("wb") as out:
            while True:
                chunk = await upload.read(1024 * 1024)
                if not chunk:
                    break
                size += len(chunk)
                if size > settings.max_file_size:
                    raise FileTooLargeError(settings.max_file_size)
                hasher.update(chunk)
                out.write(chunk)
    except FileTooLargeError:
        destination.unlink(missing_ok=True)
        raise
    except Exception:
        destination.unlink(missing_ok=True)
        raise
    finally:
        await upload.close()

    return {
        "id": uuid.uuid4().hex,
        "filename": destination.name,
        "path": str(destination),
        "size": size,
        "sha256": hasher.hexdigest(),
        "content_type": upload.content_type,
        "received_at": datetime.now(timezone.utc).isoformat(),
    }


async def list_transfers(
    db: aiosqlite.Connection,
    status: str = "all",
    limit: int = 20,
    offset: int = 0,
    session_id: str | None = None,
    peer_id: str | None = None,
) -> TransferList:
    limit = min(max(1, limit), 100)
    offset = max(0, offset)

    where_clauses: list[str] = []
    params: list = []

    if status != "all":
        where_clauses.append("status = ?")
        params.append(status)
    if session_id:
        where_clauses.append("session_id = ?")
        params.append(session_id)
    if peer_id:
        where_clauses.append("(sender_peer_id = ? OR receiver_peer_id = ?)")
        params.extend([peer_id, peer_id])

    where = ("WHERE " + " AND ".join(where_clauses)) if where_clauses else ""

    count_row = await db.execute_fetchall(
        f"SELECT COUNT(*) as cnt FROM transfer_audit {where}", params
    )
    total = count_row[0][0] if count_row else 0

    rows = await db.execute_fetchall(
        f"SELECT * FROM transfer_audit {where} ORDER BY initiated_at DESC LIMIT ? OFFSET ?",
        params + [limit, offset],
    )

    transfers = [_row_to_transfer(r) for r in rows]
    return TransferList(transfers=transfers, total=total, limit=limit, offset=offset)


async def get_transfer(db: aiosqlite.Connection, transfer_id: str) -> TransferRecord:
    rows = await db.execute_fetchall(
        "SELECT * FROM transfer_audit WHERE id = ?", [transfer_id]
    )
    if not rows:
        raise TransferNotFoundError(transfer_id)
    return _row_to_transfer(rows[0])


async def get_transfer_status(transfer_id: str) -> TransferStatus:
    from adtp_mcp.bridge import client as bridge_client

    return await bridge_client.get_status(transfer_id)


async def get_transfer_context(
    db: aiosqlite.Connection, transfer_id: str
) -> dict | None:
    rows = await db.execute_fetchall(
        "SELECT context FROM transfer_audit WHERE id = ?", [transfer_id]
    )
    if not rows:
        raise TransferNotFoundError(transfer_id)
    raw = rows[0][0]
    if raw:
        try:
            return json.loads(raw)
        except (json.JSONDecodeError, TypeError):
            pass
    return None


async def cancel_transfer(transfer_id: str) -> bool:
    from adtp_mcp.bridge import client as bridge_client

    return await bridge_client.cancel_transfer(transfer_id)


def _row_to_transfer(row) -> TransferRecord:
    context = None
    try:
        raw = row["context"] if "context" in row.keys() else None
        if raw:
            context = json.loads(raw)
    except (json.JSONDecodeError, TypeError, IndexError):
        pass

    return TransferRecord(
        id=str(row["id"]) if "id" in row.keys() else "",
        session_id=str(row["session_id"]) if "session_id" in row.keys() else "",
        transfer_type=str(
            row["transfer_type"] if "transfer_type" in row.keys() else "send"
        ),
        filename=str(row["filename"] if "filename" in row.keys() else ""),
        mime_type=str(row["mime_type"] if "mime_type" in row.keys() else ""),
        file_size=int(row["file_size"]) if "file_size" in row.keys() else 0,
        status=str(row["status"] if "status" in row.keys() else "pending"),
        integrity_verified=bool(row["integrity_verified"])
        if "integrity_verified" in row.keys()
        else False,
        signature=str(row["signature"] if "signature" in row.keys() else ""),
        chain_hash=str(row["chain_hash"] if "chain_hash" in row.keys() else ""),
        initiated_at=str(row["initiated_at"] if "initiated_at" in row.keys() else ""),
        completed_at=str(row["completed_at"])
        if "completed_at" in row.keys() and row["completed_at"]
        else None,
        context=context,
    )
