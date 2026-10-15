from __future__ import annotations

import asyncio
import json
import uuid
from datetime import datetime, timezone

import aiosqlite

from adtp_api.errors import AdtpError

AGENT_MESSAGES_SCHEMA = """CREATE TABLE IF NOT EXISTS agent_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    input TEXT NOT NULL DEFAULT '',
    text TEXT NOT NULL DEFAULT '',
    provider TEXT,
    model TEXT,
    tool_calls TEXT NOT NULL DEFAULT '[]',
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)"""


class MessageNotFoundError(AdtpError):
    def __init__(self, message_id: str):
        super().__init__(f"Warning! Agent message not found: {message_id}", 404)


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


async def ensure_agent_messages_table(db: aiosqlite.Connection) -> None:
    await db.execute(AGENT_MESSAGES_SCHEMA)
    await db.commit()


async def _connect(db_path: str) -> aiosqlite.Connection:
    import aiosqlite

    db = await aiosqlite.connect(str(db_path))
    db.row_factory = aiosqlite.Row
    await ensure_agent_messages_table(db)
    return db


def _agent_socket_available(agent_socket: str, daemon_socket: str) -> str:
    """Prefer the desktop app's agent socket, falling back to the daemon.

    The embedded LLM agent only lives in the desktop (Tauri) process; the
    adtp-client daemon answers file/queue commands but knows nothing about
    agent messages. If the desktop isn't running we still try the daemon so
    callers get a real connection error rather than a silent choice.
    """
    from pathlib import Path

    if agent_socket and Path(agent_socket).exists():
        return agent_socket
    return daemon_socket


async def submit_message(
    db: aiosqlite.Connection,
    db_path: str,
    agent_socket: str,
    daemon_socket: str,
    message: str,
    conversation_id: str | None = None,
) -> str:
    message_id = uuid.uuid4().hex
    now = _now()
    await db.execute(
        "INSERT INTO agent_messages (id, conversation_id, status, input, text, tool_calls, created_at, updated_at) "
        "VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (message_id, conversation_id or "", "pending", message, "", "[]", now, now),
    )
    await db.commit()

    loop = asyncio.get_running_loop()
    loop.create_task(
        _dispatch_message(
            db_path, agent_socket, daemon_socket, message_id, message, conversation_id
        )
    )
    return message_id


async def _dispatch_message(
    db_path: str,
    agent_socket: str,
    daemon_socket: str,
    message_id: str,
    message: str,
    conversation_id: str | None,
) -> None:
    db = await _connect(db_path)
    try:
        await db.execute(
            "UPDATE agent_messages SET status = 'running', updated_at = ? WHERE id = ?",
            (_now(), message_id),
        )
        await db.commit()
    finally:
        await db.close()

    try:
        from adtp_mcp.bridge.base import call_socket

        data = await call_socket(
            _agent_socket_available(agent_socket, daemon_socket),
            "agent_message",
            {"message": message, "conversation_id": conversation_id},
        )
        if not isinstance(data, dict):
            data = {}
        text = (
            data.get("text")
            or data.get("message")
            or data.get("response")
            or ""
        )
        provider = data.get("provider") if isinstance(data.get("provider"), str) else None
        model = data.get("model") if isinstance(data.get("model"), str) else None
        tool_calls = data.get("tool_calls") or []
        if not isinstance(tool_calls, list):
            tool_calls = []

        db = await _connect(db_path)
        try:
            await db.execute(
                "UPDATE agent_messages SET status = 'completed', text = ?, provider = ?, model = ?, "
                "tool_calls = ?, error = NULL, updated_at = ? WHERE id = ?",
                (text, provider, model, json.dumps(tool_calls), _now(), message_id),
            )
            await db.commit()
        finally:
            await db.close()
    except Exception as exc:  # noqa: BLE001 - persist failure status for the mobile poller
        db = await _connect(db_path)
        try:
            await db.execute(
                "UPDATE agent_messages SET status = 'failed', error = ?, updated_at = ? WHERE id = ?",
                (str(exc), _now(), message_id),
            )
            await db.commit()
        finally:
            await db.close()


async def get_message(db: aiosqlite.Connection, message_id: str) -> dict:
    rows = await db.execute_fetchall(
        "SELECT * FROM agent_messages WHERE id = ?", [message_id]
    )
    if not rows:
        raise MessageNotFoundError(message_id)

    row = rows[0]
    try:
        tool_calls = json.loads(row["tool_calls"]) if row["tool_calls"] else []
    except (json.JSONDecodeError, TypeError):
        tool_calls = []
    if not isinstance(tool_calls, list):
        tool_calls = []

    return {
        "id": row["id"],
        "status": row["status"],
        "message": row["text"],
        "text": row["text"],
        "input": row["input"],
        "provider": row["provider"] if "provider" in row.keys() else None,
        "model": row["model"] if "model" in row.keys() else None,
        "tool_calls": tool_calls,
        "conversation_id": row["conversation_id"] or None,
        "error": row["error"] if "error" in row.keys() else None,
        "created_at": row["created_at"],
        "updated_at": row["updated_at"],
    }


async def get_status(db: aiosqlite.Connection, agent_socket: str, daemon_socket: str) -> dict:
    from adtp_mcp.bridge.base import call_socket

    try:
        data = await call_socket(
            _agent_socket_available(agent_socket, daemon_socket), "agent_status", {}
        )
        if not isinstance(data, dict):
            data = {}
        return {
            "active": bool(data.get("active", False)),
            "provider": (
                data.get("provider")
                if isinstance(data.get("provider"), str)
                else None
            ),
            "model": (
                data.get("model") if isinstance(data.get("model"), str) else None
            ),
            "current_task_id": (
                data.get("current_task_id")
                or data.get("currentTaskId")
                or data.get("currentConvoId")
            ),
        }
    except Exception:
        pass

    current_task_id: str | None = None
    in_flight = await db.execute_fetchall(
        "SELECT id FROM agent_messages WHERE status IN ('pending', 'running') "
        "ORDER BY created_at DESC LIMIT 1"
    )
    if in_flight:
        current_task_id = str(in_flight[0][0])

    provider = None
    model = None
    latest = await db.execute_fetchall(
        "SELECT provider, model FROM agent_messages WHERE status = 'completed' "
        "ORDER BY created_at DESC LIMIT 1"
    )
    if latest:
        row = latest[0]
        provider = row["provider"] if "provider" in row.keys() else None
        model = row["model"] if "model" in row.keys() else None

    return {
        "active": current_task_id is not None,
        "provider": provider,
        "model": model,
        "current_task_id": current_task_id,
    }