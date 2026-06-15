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


class SendFileResult(BaseModel):
    transfer_id: str
    status: str
    filename: str
    file_size: int
    estimated_ms: int = Field(default=0, ge=0)
    context: dict | None = None


async def send_file(input_data: SendFileInput) -> dict:
    validate_all_strings(
        (
            ("file_path", input_data.file_path),
            ("agent_name", input_data.agent_name),
            ("message", input_data.message),
            ("priority", input_data.priority),
        )
    )
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
    )
    return result.model_dump()
