from __future__ import annotations

from pydantic import BaseModel

from adtp_mcp.bridge import client as client_bridge
from adtp_mcp.security.validator import validate_uuid


class GetFileContextInput(BaseModel):
    transfer_id: str


async def get_file_context(input_data: GetFileContextInput) -> dict:
    transfer_id = validate_uuid(input_data.transfer_id, "transfer_id")
    return await client_bridge.get_context(transfer_id)
