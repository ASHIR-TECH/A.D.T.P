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
    session_id: str = ""
    peer_id: str | None = None


class ReceiveFileResult(BaseModel):
    transfer_id: str
    status: str
    filename: str
    delivered_to: str | None = None
    context: dict | None = None
    peer_id: str | None = None


async def receive_file(input_data: ReceiveFileInput) -> dict:
    validate_all_strings(
        (
            ("file_path", input_data.file_path),
            ("destination_folder", input_data.destination_folder),
            ("message", input_data.message),
        )
    )
    if input_data.peer_id and not _is_valid_peer_id(input_data.peer_id):
        raise ValueError(f"Invalid peer_id: {input_data.peer_id}")
    file_path = validate_file_for_receive(input_data.file_path)
    context = await build_context(file_path, input_data.session_id)
    bridge_response = await server_bridge.push_file(
        str(file_path), input_data.destination_folder, context
    )
    result = ReceiveFileResult(
        transfer_id=str(bridge_response.get("transfer_id", "")),
        status=str(bridge_response.get("status", "queued")),
        filename=Path(file_path).name,
        delivered_to=bridge_response.get("delivered_to")
        or input_data.destination_folder,
        context=context,
        peer_id=input_data.peer_id,
    )
    return result.model_dump()


def _is_valid_peer_id(peer_id: str) -> bool:
    if len(peer_id) != 32:
        return False
    try:
        int(peer_id, 16)
        return True
    except ValueError:
        return False
