from __future__ import annotations

import asyncio
import json
import sys
from pathlib import Path
from typing import Any

from adtp_mcp.config import settings


class BridgeError(RuntimeError):
    pass


async def call_socket(socket_path: Path, cmd: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
    payload = {"cmd": cmd, "params": params or {}}
    timeout = settings.bridge_timeout_ms / 1000

    if sys.platform == "win32":
        raise BridgeError("Windows named-pipe bridge is not implemented in Phase 2")

    try:
        reader, writer = await asyncio.wait_for(asyncio.open_unix_connection(str(socket_path)), timeout)
    except (OSError, TimeoutError) as exc:
        raise BridgeError(f"Daemon unreachable at {socket_path}: {exc}") from exc

    try:
        writer.write(json.dumps(payload, separators=(",", ":")).encode() + b"\n")
        await asyncio.wait_for(writer.drain(), timeout)
        line = await asyncio.wait_for(reader.readline(), timeout)
    finally:
        writer.close()
        await writer.wait_closed()

    if not line:
        raise BridgeError("Daemon returned an empty response")

    try:
        response = json.loads(line.decode())
    except json.JSONDecodeError as exc:
        raise BridgeError("Daemon returned invalid JSON") from exc

    if not response.get("ok", False):
        raise BridgeError(str(response.get("error", "Unknown daemon error")))

    data = response.get("data", {})
    if not isinstance(data, dict):
        raise BridgeError("Daemon response data must be an object")
    return data
