from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, Field

from adtp_mcp.bridge import client as client_bridge


class ListTransfersInput(BaseModel):
    status: Literal["pending", "delivered", "failed", "all"] = "all"
    limit: int = Field(default=10, ge=1, le=100)


async def list_transfers(input_data: ListTransfersInput) -> list[dict]:
    response = await client_bridge.get_transfers(input_data.status, input_data.limit)
    transfers = response.get("transfers", [])
    if not isinstance(transfers, list):
        return []
    return transfers
