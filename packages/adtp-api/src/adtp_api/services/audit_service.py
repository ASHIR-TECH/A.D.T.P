from __future__ import annotations

import csv
import hashlib
import io
from datetime import datetime, timezone

import aiosqlite

from adtp_api.errors import TransferNotFoundError
from adtp_api.models.audit import AuditEntry, AuditLogList, ChainVerifyResult

# The Rust client writes its local `transfers` table; this is the gateway's
# own audit log, which also records files the phone uploads. It is created on
# demand so History/Downloads work on a fresh install (previously the table was
# only assumed to exist and every audit query failed with "no such table").
TRANSFER_AUDIT_SCHEMA = """CREATE TABLE IF NOT EXISTS transfer_audit (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL DEFAULT '',
    transfer_type TEXT NOT NULL DEFAULT 'send',
    filename TEXT NOT NULL DEFAULT '',
    mime_type TEXT NOT NULL DEFAULT '',
    file_size INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending',
    integrity_verified INTEGER NOT NULL DEFAULT 0,
    signature TEXT NOT NULL DEFAULT '',
    chain_hash TEXT NOT NULL DEFAULT '',
    initiated_at TEXT NOT NULL,
    completed_at TEXT,
    sender_peer_id TEXT NOT NULL DEFAULT '',
    receiver_peer_id TEXT NOT NULL DEFAULT '',
    context TEXT,
    timestamp TEXT NOT NULL DEFAULT '',
    agent_action TEXT NOT NULL DEFAULT ''
)"""


async def ensure_transfer_audit_table(db: aiosqlite.Connection) -> None:
    await db.execute(TRANSFER_AUDIT_SCHEMA)
    await db.commit()


def chain_digest(previous_hash: str, transfer_id: str) -> str:
    """Hash used for a gateway audit row, matching `verify_chain` below."""
    data = f"{previous_hash}{transfer_id}".encode()
    try:
        import blake3  # type: ignore[import-not-found]

        return blake3.blake3(data).hexdigest()
    except ImportError:  # pragma: no cover - only when optional wheel missing
        return hashlib.sha256(data).hexdigest()


async def append_transfer_audit(db: aiosqlite.Connection, row: dict) -> None:
    """Insert an audit row, chaining it to the previous gateway entry."""
    previous = await db.execute_fetchall(
        "SELECT chain_hash FROM transfer_audit ORDER BY rowid DESC LIMIT 1"
    )
    previous_hash = str(previous[0][0]) if previous else ""
    transfer_id = str(row["id"])
    await db.execute(
        "INSERT INTO transfer_audit (id, session_id, transfer_type, filename, "
        "mime_type, file_size, status, integrity_verified, signature, "
        "chain_hash, initiated_at, completed_at, sender_peer_id, "
        "receiver_peer_id, context, timestamp, agent_action) "
        "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            transfer_id,
            row.get("session_id", ""),
            row.get("transfer_type", "send"),
            row.get("filename", ""),
            row.get("mime_type", ""),
            int(row.get("file_size", 0)),
            row.get("status", "delivered"),
            int(bool(row.get("integrity_verified", True))),
            row.get("signature", ""),
            chain_digest(previous_hash, transfer_id),
            row.get("initiated_at", ""),
            row.get("completed_at"),
            row.get("sender_peer_id", ""),
            row.get("receiver_peer_id", ""),
            row.get("context"),
            row.get("timestamp", row.get("initiated_at", "")),
            row.get("agent_action", "received"),
        ),
    )
    await db.commit()


async def query_log(
    db: aiosqlite.Connection,
    session_id: str | None = None,
    transfer_type: str | None = None,
    status: str | None = None,
    from_ts: str | None = None,
    to_ts: str | None = None,
    limit: int = 20,
    offset: int = 0,
) -> AuditLogList:
    limit = min(max(1, limit), 100)
    offset = max(0, offset)

    where_clauses: list[str] = []
    params: list = []

    if session_id:
        where_clauses.append("session_id = ?")
        params.append(session_id)
    if transfer_type:
        where_clauses.append("transfer_type = ?")
        params.append(transfer_type)
    if status:
        where_clauses.append("status = ?")
        params.append(status)
    if from_ts:
        where_clauses.append("initiated_at >= ?")
        params.append(from_ts)
    if to_ts:
        where_clauses.append("initiated_at <= ?")
        params.append(to_ts)

    where = ("WHERE " + " AND ".join(where_clauses)) if where_clauses else ""

    count_row = await db.execute_fetchall(
        f"SELECT COUNT(*) as cnt FROM transfer_audit {where}", params
    )
    total = count_row[0][0] if count_row else 0

    rows = await db.execute_fetchall(
        f"SELECT * FROM transfer_audit {where} ORDER BY initiated_at DESC LIMIT ? OFFSET ?",
        params + [limit, offset],
    )

    entries = [_row_to_audit_entry(r) for r in rows]
    return AuditLogList(entries=entries, total=total)


async def get_entry(db: aiosqlite.Connection, transfer_id: str) -> AuditEntry:
    rows = await db.execute_fetchall(
        "SELECT * FROM transfer_audit WHERE id = ?", [transfer_id]
    )
    if not rows:
        raise TransferNotFoundError(transfer_id)
    return _row_to_audit_entry(rows[0])


async def export_csv(
    db: aiosqlite.Connection,
    session_id: str | None = None,
    from_ts: str | None = None,
    to_ts: str | None = None,
) -> str:
    where_clauses: list[str] = []
    params: list = []

    if session_id:
        where_clauses.append("session_id = ?")
        params.append(session_id)
    if from_ts:
        where_clauses.append("initiated_at >= ?")
        params.append(from_ts)
    if to_ts:
        where_clauses.append("initiated_at <= ?")
        params.append(to_ts)

    where = ("WHERE " + " AND ".join(where_clauses)) if where_clauses else ""

    rows = await db.execute_fetchall(
        f"SELECT * FROM transfer_audit {where} ORDER BY initiated_at ASC", params
    )

    output = io.StringIO()
    if not rows:
        return output.getvalue()

    fieldnames = [d[0] for d in db.description()] if rows else []
    writer = csv.DictWriter(output, fieldnames=fieldnames)
    writer.writeheader()
    for r in rows:
        writer.writerow(dict(r))
    return output.getvalue()


async def verify_chain(db: aiosqlite.Connection) -> ChainVerifyResult:
    rows = await db.execute_fetchall(
        "SELECT id, chain_hash, signature, session_id, filename FROM transfer_audit ORDER BY rowid ASC"
    )

    entries_checked = 0
    first_failure_id: str | None = None
    prev_hash = ""

    for r in rows:
        transfer_id = str(r["id"])
        chain_hash = str(r["chain_hash"])

        expected = chain_digest(prev_hash, transfer_id)
        if chain_hash != expected:
            first_failure_id = transfer_id
            break

        prev_hash = chain_hash
        entries_checked += 1

    now = datetime.now(timezone.utc).isoformat()
    return ChainVerifyResult(
        valid=first_failure_id is None,
        entries_checked=entries_checked,
        first_failure_id=first_failure_id,
        verified_at=now,
    )


def _row_to_audit_entry(row) -> AuditEntry:
    return AuditEntry(
        id=str(row["id"]) if "id" in row.keys() else "",
        session_id=str(row["session_id"]) if "session_id" in row.keys() else "",
        transfer_type=str(
            row["transfer_type"] if "transfer_type" in row.keys() else "send"
        ),
        filename=str(row["filename"] if "filename" in row.keys() else ""),
        mime_type=str(row["mime_type"] if "mime_type" in row.keys() else ""),
        file_size=int(row["file_size"]) if "file_size" in row.keys() else 0,
        status=str(row["status"] if "status" in row.keys() else ""),
        integrity_verified=bool(row["integrity_verified"])
        if "integrity_verified" in row.keys()
        else False,
        signature=str(row["signature"] if "signature" in row.keys() else ""),
        chain_hash=str(row["chain_hash"] if "chain_hash" in row.keys() else ""),
        initiated_at=str(row["initiated_at"] if "initiated_at" in row.keys() else ""),
        completed_at=str(row["completed_at"])
        if "completed_at" in row.keys() and row["completed_at"]
        else None,
    )
