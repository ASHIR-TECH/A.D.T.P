from __future__ import annotations

from pydantic import BaseModel


class TransferHistoryEntry(BaseModel):
    timestamp: str
    mime_type: str
    agent_action: str = ""


class ContextBlock(BaseModel):
    schema_: dict | None = None
    summary: str | None = None
    agent_hint: str | None = None
    transfer_history: list[TransferHistoryEntry] = []
    extraction_ms: int | None = None
