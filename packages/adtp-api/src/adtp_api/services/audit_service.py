from __future__ import annotations

import csv
import hashlib
import io
from datetime import datetime, timezone

import aiosqlite

from adtp_api.errors import TransferNotFoundError
from adtp_api.models.audit import AuditEntry, AuditLogList, ChainVerifyResult


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

        expected = hashlib.blake3(f"{prev_hash}{transfer_id}".encode()).hexdigest()
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
