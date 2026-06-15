from __future__ import annotations

from pydantic import BaseModel


class SessionInfo(BaseModel):
    id: str
    peer_addr: str = ""
    started_at: str
    ended_at: str | None = None
    transfer_count: int = 0
    bytes_transferred: int = 0


class SessionList(BaseModel):
    sessions: list[SessionInfo]
    total: int
