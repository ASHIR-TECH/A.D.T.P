from __future__ import annotations

from adtp_mcp.bridge import client as client_bridge

URI = "adtp://transfers/log"
MIME_TYPE = "application/json"


async def read() -> dict:
    return await client_bridge.get_transfers("all", 100)
