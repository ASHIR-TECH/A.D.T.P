from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, Field

from adtp_mcp.bridge import client as client_bridge
from adtp_mcp.context.packager import build_context
from adtp_mcp.security.validator import validate_all_strings, validate_file_for_send


class SendFileInput(BaseModel):
    file_path: str
    agent_name: str = "default"
    message: str | None = None
    priority: Literal["normal", "urgent"] = "normal"
    session_id: str = ""
    peer_id: str | None = None


class SendFileResult(BaseModel):
    transfer_id: str
    status: str
    filename: str
    file_size: int
    estimated_ms: int = Field(default=0, ge=0)
    context: dict | None = None
    peer_id: str | None = None


async def send_file(input_data: SendFileInput) -> dict:
    validate_all_strings(
        (
            ("file_path", input_data.file_path),
            ("agent_name", input_data.agent_name),
            ("message", input_data.message),
            ("priority", input_data.priority),
        )
    )
    if input_data.peer_id and not _is_valid_peer_id(input_data.peer_id):
        raise ValueError(f"Invalid peer_id: {input_data.peer_id}")
    file_path = validate_file_for_send(input_data.file_path)
    context = await build_context(file_path, input_data.session_id)
    bridge_response = await client_bridge.enqueue_send(
        str(file_path), input_data.agent_name, input_data.priority, context
    )
    result = SendFileResult(
        transfer_id=str(bridge_response.get("transfer_id", "")),
        status=str(bridge_response.get("status", "queued")),
        filename=file_path.name,
        file_size=file_path.stat().st_size,
        estimated_ms=int(bridge_response.get("estimated_ms", 0)),
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
