from __future__ import annotations

from pathlib import Path

from pydantic import BaseModel

from adtp_mcp.bridge import server as server_bridge
from adtp_mcp.context.packager import build_context
from adtp_mcp.security.validator import validate_all_strings, validate_file_for_receive


class ReceiveFileInput(BaseModel):
    file_path: str
    destination_folder: str | None = None
    message: str | None = None


class ReceiveFileResult(BaseModel):
    transfer_id: str
    status: str
    filename: str
    delivered_to: str | None = None


async def receive_file(input_data: ReceiveFileInput) -> dict:
    validate_all_strings(
        (
            ("file_path", input_data.file_path),
            ("destination_folder", input_data.destination_folder),
            ("message", input_data.message),
        )
    )
    file_path = validate_file_for_receive(input_data.file_path)
    build_context(file_path)
    bridge_response = await server_bridge.push_file(str(file_path), input_data.destination_folder)
    result = ReceiveFileResult(
        transfer_id=str(bridge_response.get("transfer_id", "")),
        status=str(bridge_response.get("status", "queued")),
        filename=Path(file_path).name,
        delivered_to=bridge_response.get("delivered_to") or input_data.destination_folder,
    )
    return result.model_dump()
