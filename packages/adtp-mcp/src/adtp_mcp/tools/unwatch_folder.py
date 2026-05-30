from __future__ import annotations

from pydantic import BaseModel, model_validator

from adtp_mcp.bridge import client as client_bridge
from adtp_mcp.security.validator import validate_absolute_path, validate_all_strings, validate_uuid


class UnwatchFolderInput(BaseModel):
    watch_id: str | None = None
    folder_path: str | None = None

    @model_validator(mode="after")
    def require_identifier(self) -> "UnwatchFolderInput":
        if not self.watch_id and not self.folder_path:
            raise ValueError("watch_id or folder_path is required")
        return self


class UnwatchFolderResult(BaseModel):
    watch_id: str | None = None
    removed: bool


async def unwatch_folder(input_data: UnwatchFolderInput) -> dict:
    validate_all_strings((("watch_id", input_data.watch_id), ("folder_path", input_data.folder_path)))
    watch_id = validate_uuid(input_data.watch_id, "watch_id") if input_data.watch_id else None
    folder_path = str(validate_absolute_path(input_data.folder_path, "folder_path")) if input_data.folder_path else None
    bridge_response = await client_bridge.remove_watch(watch_id, folder_path)
    return UnwatchFolderResult(
        watch_id=bridge_response.get("watch_id") or watch_id,
        removed=bool(bridge_response.get("removed", False)),
    ).model_dump()
