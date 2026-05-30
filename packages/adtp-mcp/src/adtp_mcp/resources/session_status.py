from __future__ import annotations

from adtp_mcp.bridge import client as client_bridge

URI = "adtp://session/status"
MIME_TYPE = "application/json"


async def read() -> dict:
    return await client_bridge.session_status()
