from __future__ import annotations

from pydantic import BaseModel

from adtp_mcp.bridge import client as client_bridge
from adtp_mcp.security.validator import validate_uuid


class TransferStatusInput(BaseModel):
    transfer_id: str


async def transfer_status(input_data: TransferStatusInput) -> dict:
    transfer_id = validate_uuid(input_data.transfer_id, "transfer_id")
    response = await client_bridge.get_status(transfer_id)
    total = int(response.get("total_bytes", 0) or 0)
    sent = int(response.get("bytes_sent", 0) or 0)
    percent = (sent / total * 100.0) if total else 0.0
    return {
        "status": response.get("status", "unknown"),
        "bytes_sent": sent,
        "total_bytes": total,
        "percent": percent,
        "elapsed_ms": int(response.get("elapsed_ms", 0) or 0),
    }
