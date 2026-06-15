from __future__ import annotations

from typing import Any

from adtp_mcp.bridge.base import call_socket
from adtp_mcp.config import settings


async def enqueue_send(
    file_path: str,
    agent_name: str = "default",
    priority: str = "normal",
    context: dict[str, Any] | None = None,
) -> dict[str, Any]:
    params: dict[str, Any] = {
        "file_path": file_path,
        "agent_name": agent_name,
        "priority": priority,
    }
    if context:
        params["context"] = context
    return await call_socket(settings.client_socket, "send", params)


async def register_watch(
    folder_path: str, pattern: str | None = None, action: str | None = None
) -> dict[str, Any]:
    return await call_socket(
        settings.client_socket,
        "watch",
        {"folder_path": folder_path, "pattern": pattern, "trigger_action": action},
    )


async def remove_watch(
    watch_id: str | None = None, folder_path: str | None = None
) -> dict[str, Any]:
    return await call_socket(
        settings.client_socket,
        "unwatch",
        {"watch_id": watch_id, "folder_path": folder_path},
    )


async def get_transfers(status: str = "all", limit: int = 10) -> dict[str, Any]:
    return await call_socket(
        settings.client_socket,
        "get_transfers",
        {"status_filter": status, "limit": limit},
    )


async def get_context(transfer_id: str) -> dict[str, Any]:
    return await call_socket(
        settings.client_socket, "get_context", {"transfer_id": transfer_id}
    )


async def get_status(transfer_id: str) -> dict[str, Any]:
    return await call_socket(
        settings.client_socket, "get_status", {"transfer_id": transfer_id}
    )


async def session_status() -> dict[str, Any]:
    return await call_socket(settings.client_socket, "session_status", {})


async def pending_files() -> dict[str, Any]:
    return await call_socket(settings.client_socket, "pending_files", {})
