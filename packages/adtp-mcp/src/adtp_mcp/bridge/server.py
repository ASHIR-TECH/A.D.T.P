from __future__ import annotations

from typing import Any

from adtp_mcp.bridge.base import call_socket
from adtp_mcp.config import settings


async def push_file(file_path: str, destination_folder: str | None = None) -> dict[str, Any]:
    return await call_socket(settings.server_socket, "push", {"file_path": file_path, "destination": destination_folder})


async def session_status() -> dict[str, Any]:
    return await call_socket(settings.server_socket, "session_status", {})
