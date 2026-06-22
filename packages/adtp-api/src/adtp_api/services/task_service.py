from __future__ import annotations

import json
from datetime import datetime, timezone
from uuid import uuid4

import aiosqlite

from adtp_api.models.task import TaskList, TaskRecord


async def create_task(
    db: aiosqlite.Connection,
    peer_id: str,
    file_path: str,
    instructions: str,
    expected_artifacts: list[str],
) -> dict:
    task_id = str(uuid4())
    transfer_id = str(uuid4())
    now = datetime.now(timezone.utc).isoformat()

    await db.execute(
        """INSERT INTO tasks
           (task_id, transfer_id, sender_peer_id, receiver_peer_id, instructions,
            expected_artifacts, state, reason, artifacts_json, created_at, updated_at)
           VALUES (?, ?, '', ?, ?, ?, 'received', NULL, '[]', ?, ?)""",
        (
            task_id,
            transfer_id,
            peer_id,
            instructions,
            json.dumps(expected_artifacts),
            now,
            now,
        ),
    )
    await db.commit()

    return {
        "task_id": task_id,
        "transfer_id": transfer_id,
        "state": "received",
        "instructions": instructions,
    }


async def list_tasks(
    db: aiosqlite.Connection,
    peer_id: str | None = None,
    state: str | None = None,
    limit: int = 20,
    offset: int = 0,
) -> TaskList:
    where_clauses: list[str] = []
    params: list = []

    if peer_id:
        where_clauses.append("(sender_peer_id = ? OR receiver_peer_id = ?)")
        params.extend([peer_id, peer_id])
    if state:
        where_clauses.append("state = ?")
        params.append(state)

    where = ("WHERE " + " AND ".join(where_clauses)) if where_clauses else ""

    rows = await db.execute_fetchall(
        f"SELECT * FROM tasks {where} ORDER BY created_at DESC LIMIT ? OFFSET ?",
        params + [limit, offset],
    )
    total_rows = await db.execute_fetchall(
        f"SELECT COUNT(*) as cnt FROM tasks {where}", params
    )
    total = total_rows[0][0] if total_rows else 0

    tasks = [_row_to_task(r) for r in rows]
    return TaskList(tasks=tasks, total=total, limit=limit, offset=offset)


async def get_task(db: aiosqlite.Connection, task_id: str) -> dict:
    rows = await db.execute_fetchall(
        "SELECT * FROM tasks WHERE task_id = ?", [task_id]
    )
    if not rows:
        return {"error": "task not found"}
    task = _row_to_task(rows[0])
    return task.model_dump()


async def transition_task(
    db: aiosqlite.Connection,
    task_id: str,
    new_state: str,
    reason: str | None = None,
) -> dict:
    rows = await db.execute_fetchall(
        "SELECT * FROM tasks WHERE task_id = ?", [task_id]
    )
    if not rows:
        return {"error": "task not found", "task_id": task_id}

    current_state = rows[0]["state"] if "state" in rows[0].keys() else "received"
    now = datetime.now(timezone.utc).isoformat()

    await db.execute(
        "UPDATE tasks SET state = ?, reason = ?, updated_at = ? WHERE task_id = ?",
        (new_state, reason, now, task_id),
    )
    await db.commit()

    return {
        "task_id": task_id,
        "state": new_state,
        "previous_state": current_state,
        "reason": reason,
    }


async def report_artifact(
    db: aiosqlite.Connection,
    task_id: str,
    file_path: str,
    artifact_name: str,
) -> dict:
    rows = await db.execute_fetchall(
        "SELECT artifacts_json FROM tasks WHERE task_id = ?", [task_id]
    )
    if not rows:
        return {"error": "task not found", "task_id": task_id}

    raw = rows[0][0]
    artifacts = json.loads(raw) if raw else []
    transfer_id = str(uuid4())
    now = datetime.now(timezone.utc).isoformat()

    artifact_entry = {
        "artifact_name": artifact_name,
        "transfer_id": transfer_id,
        "file_path": file_path,
        "produced_at": now,
    }
    artifacts.append(artifact_entry)

    await db.execute(
        "UPDATE tasks SET artifacts_json = ?, state = 'artifact_ready', updated_at = ? WHERE task_id = ?",
        (json.dumps(artifacts), now, task_id),
    )
    await db.commit()

    return {
        "task_id": task_id,
        "artifact_name": artifact_name,
        "transfer_id": transfer_id,
        "state": "artifact_ready",
    }


def _row_to_task(row) -> TaskRecord:
    artifacts_raw = row["artifacts_json"] if "artifacts_json" in row.keys() else "[]"
    try:
        artifacts_list = json.loads(artifacts_raw) if artifacts_raw else []
    except (json.JSONDecodeError, TypeError):
        artifacts_list = []

    return TaskRecord(
        task_id=str(row["task_id"]) if "task_id" in row.keys() else "",
        transfer_id=str(row["transfer_id"]) if "transfer_id" in row.keys() else "",
        session_id=str(row["session_id"]) if "session_id" in row.keys() else "",
        sender_peer_id=str(row["sender_peer_id"])
        if "sender_peer_id" in row.keys()
        else "",
        receiver_peer_id=str(row["receiver_peer_id"])
        if "receiver_peer_id" in row.keys()
        else "",
        instructions=str(row["instructions"])
        if "instructions" in row.keys()
        else "",
        expected_artifacts=json.loads(row["expected_artifacts"])
        if "expected_artifacts" in row.keys() and row["expected_artifacts"]
        else [],
        state=str(row["state"]) if "state" in row.keys() else "received",
        reason=str(row["reason"]) if "reason" in row.keys() else None,
        artifacts=[_dict_to_artifact(a) for a in artifacts_list],
        created_at=str(row["created_at"]) if "created_at" in row.keys() else "",
        updated_at=str(row["updated_at"]) if "updated_at" in row.keys() else "",
    )


def _dict_to_artifact(a: dict) -> dict:
    return {
        "artifact_name": a.get("artifact_name", ""),
        "transfer_id": a.get("transfer_id", ""),
        "mime_type": a.get("mime_type", "application/octet-stream"),
        "file_size": a.get("file_size", 0),
        "produced_at": a.get("produced_at", ""),
    }
