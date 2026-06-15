from __future__ import annotations

from pydantic import BaseModel


class WatchRequest(BaseModel):
    folder_path: str
    file_pattern: str | None = None
    trigger_action: str | None = None
    agent_name: str = "default"


class WatchResponse(BaseModel):
    watch_id: str
    folder_path: str
    file_pattern: str | None = None
    trigger_action: str | None = None
    active: bool = True
    agent_name: str = "default"


class WatchList(BaseModel):
    watchers: list[WatchResponse]
    total: int
