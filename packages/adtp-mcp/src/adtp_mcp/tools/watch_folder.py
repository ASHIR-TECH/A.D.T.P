from __future__ import annotations

from pydantic import BaseModel

from adtp_mcp.bridge import client as client_bridge
from adtp_mcp.security.validator import validate_all_strings, validate_folder


class WatchFolderInput(BaseModel):
    folder_path: str
    file_pattern: str | None = None
    trigger_action: str | None = None


class WatchFolderResult(BaseModel):
    watch_id: str
    folder_path: str
    pattern: str | None = None
    active: bool


async def watch_folder(input_data: WatchFolderInput) -> dict:
    validate_all_strings(
        (
            ("folder_path", input_data.folder_path),
            ("file_pattern", input_data.file_pattern),
            ("trigger_action", input_data.trigger_action),
        )
    )
    folder = validate_folder(input_data.folder_path)
    bridge_response = await client_bridge.register_watch(str(folder), input_data.file_pattern, input_data.trigger_action)
    return WatchFolderResult(
        watch_id=str(bridge_response.get("watch_id", "")),
        folder_path=str(folder),
        pattern=input_data.file_pattern,
        active=bool(bridge_response.get("active", True)),
    ).model_dump()
